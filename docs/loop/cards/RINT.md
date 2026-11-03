---
id: RINT
title: Render interpolation: the sim stays 30 Hz, the windowed game draws positions, the camera and animation blended between the last two ticks at the display's rate (the maintainer approved 10-20; after HITCH's recorder) - today a 60 / 144 Hz frame repeats the last tick, so motion steps
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
The maintainer, 2026-10-20 17:05 (INBOX, standing for PERF / HITCH / RINT): "(a) approved: RINT, the sim stays 30 Hz and the drawn
positions, camera and animation interpolate between the last two ticks at the display's rate (open the card after HITCH's
recorder)." Today the windowed game draws the latest 30 Hz tick as it is: at 60 / 144 Hz a frame repeats the last tick's
picture, so motion steps (no interpolation between ticks was found when HITCH was carded).

## Evidence
`game/src/walk.rs:804` `advance_with` (the fixed-step accumulator; `accum` left over after the ticks is the blend factor),
`game/src/main/windowed.rs:1443` (the frame loop), the draw-list builders that read unit / vehicle / bolt / camera state
(`soldier_vis.rs`, `viewer/vehicles.rs`, `bolt_vis.rs`, the camera). HITCH's recorder (Part 1) gives the frame split.

## What
Keep the previous tick's drawn state (unit and vehicle transforms, the camera, animation clip times, bolts' heads) and draw
`lerp(prev, cur, accum / TICK)` (slerp for rotations; a teleport, respawn or seat change snaps, no blend across it). The
sim never reads the blend: it is presentation only. The player's own camera turn keeps its input latency (the look input
applied at the display's rate if the PS2-measured feel allows; FEEL_NOTES line either way).

## Notes
Opened by the planner 2026-10-20 17:50 on the maintainer's approval; waits on HITCH (deps) so its recorder can measure it.
## Landed 2026-10-21 17:55 (982999c, lane 1; report `reports/20261021_1756_card_RINT.md`)
`game::render_blend`: the pose before each tick kept (`WalkSim::advance_with`), the frame's draw lists built with the
units' feet / yaw / pitch, the vehicles' place / turn / gun looks / walker aim and the zoom ease blended at accum / TICK
(`WalkSim::blend_in`), restored bit for bit before anything else reads the walk (`blend_out`); a new generation, an alive
flip, a seat change or a jump over 5 m (unit) / 10 m (vehicle) snaps. Bolts' heads blend in `BoltVis::draws`.
Acceptance, KAS2 bf2, 70 s autopilot, no vsync (~1100 fps), RTX, `lead_tmp/rint/stood.py` on the frame CSV's new
cam_* / me_* columns, frames resampled at the display rate, past 5 s:
- 144 Hz: frames drawing the camera where the frame before did, while the player moved: 7376 / 9326 (79.1 %) -> 0 / 9220
- 60 Hz: 1950 / 3900 (50.0 %) -> 0 / 3901; every frame: 97.3 % -> 0
- even motion (`jitter.py`, 144 Hz): a frame's camera step against its 1/15 s mean, p50 deviation 1.00 -> 0.05, p90 5.00 -> 0.13
Before = the same binary with `OPENBF_RENDER_BLEND=0` (`lead_tmp/hitch/win/bf2_kas2_rint_blend{0,1}/`).
Not blended (stepping at 30 Hz still): a walker's posed legs, doors / stations, a bolt's drawn START (the fire point read
at its tick: up to ~0.2 m off a running shooter's drawn gun for a frame), the spawn backdrop.
