# PERF-1 result: automatic quality reads split frames (2026-11-27)

**Status: partial delivery.** Delivered: `acquire_ms` is correct in split, the `acquire_seen`
latch no longer hides split frames on Linux, the CPU-bound rule takes the wait into account, and
the session-log lines exist. Not delivered: the scale-selection gain. kas2 XL holds 1.0 although
lower scales have a lower p95 (see Deviations), and `WAIT_SHARE` is untested under the window's
Fifo. **Acceptance change, needs the maintainer's sign-off before landing:** the item "kas2 XL
settles on the scale with the lower p95" is dropped from PERF-1 and moves to a separate,
reviewed follow-up packet (a measured trial step on CPU-bound windows). If that change is not
accepted, PERF-1 does not land as is.

## What changed

- `render` (frame/submit.rs): `acquire_ms` is written only by a present's first view, the one
  that acquires the window's image. Later split views take the held image and no longer
  overwrite the wait with about 0. A single view is always first, so one view is unchanged.
  `Renderer::acquire_delay` adds a wait to the first view's acquire, for the headless test. It
  and its sleep exist only with render's `test-support` feature, which only render's own tests
  enable (a dev-dependency on itself), so the game's renderer has neither.
- Window (events.rs): where the thread CPU clock exists (Linux), every governed frame passes
  busy = the main thread's CPU ms since the last governed frame (`CLOCK_THREAD_CPUTIME_ID`, now
  `game::frame_profile::thread_cpu_ms`, shared with the bench); the `acquire_seen` latch does
  not apply there. Without the clock (Windows, macOS) the old rule stays: busy = frame time less
  the acquire wait, and only after one acquire wait above 2 ms has been seen; until then busy is
  `None`, so a driver that blocks in present rather than acquire is never called CPU-bound. The
  acquire wait is passed as well.
- `quality_governor`: `frame(dt, busy, wait, budget, tiers)`. A window counts as CPU-bound only
  when busy p95 is over budget x `OVER` **and** the wait p95 is at most budget x `WAIT_SHARE`
  (0.25, 4.2 ms at 60 Hz). An over-budget CPU that still waits for the image steps down. This is
  a logic fix. `OVER`, `HEADROOM`, the holds and the windows are unchanged. The CPU-bound
  summary counts only frames whose window was over budget and CPU-bound, not the band after one.
- Session log (`Note`, `Governor::take_notes`; the window and the bench print them with
  `quality_governor::map_label`, the map dir plus ` xl`, and the view count):
  `quality: map bf2_kas2 xl views 4 scale 0.85 tier -0 steps 1 cpu_bound false p95 .. busy p95 .. wait p95 ..`
  for each scale or tier change, `quality: map .. views .. step up to scale 1.00 tier -0 failed, p95 ..`
  for each failed trial, and `quality: map .. views .. cpu_bound S of 60 s, p95 ..` at most once
  a minute, only when a window was CPU-bound. Nothing is logged per window.
- `frame_profile::FrameRow` gains `world_scale` (serde default 0 for older records). It is shown
  in the hitch line when below 1.
- Bench: `--bench-governor` (detailed bench, pipelined only) drives the scale through the
  governor every frame. Inputs are the last frame's time, main-thread CPU and its wait for a
  frame in flight (headless, that wait stands in for the acquire), held to 60 Hz. The summary
  gains `governor.changes`, `final_scale` and frame p50/p95/p99 per scale. The bench prints the
  governor's notes too.

## Tests

- `scripts/one_player_gate.py`: every row matches (lan_soak ok, 1559 ticks, 4312 hash checks).
- `cargo test -p render --test suite acquire_ms -- --ignored`: 1 passed. It is `#[ignore]` (it
  needs a GPU adapter) like the suite's other 96 GPU tests, so a plain `cargo test` skips it;
  it runs with `--ignored` on a machine with a GPU
  (`views::acquire_ms_is_the_first_views_wait`: 4 views with a 20 ms wait on view 0, so
  `acquire_ms()` is 20 ms after every view; one-view presents write their own wait). With the
  old unconditional write it fails: "after view 1: 0.000319 ms".
- `cargo test -p game --lib -- quality_governor frame_profile`: 18 + 1 passed. That covers 13
  governor tests, 2 of them new:
  - `cpu_over_with_a_gpu_wait_steps_down` (a synthetic mixed case: dt 33, busy 24, wait 9). It
    is not kas2 XL: the planned 29.8 ms GPU figure it came from is contradicted by the bench
    (GPU 18.7 ms pipelined), and under Fifo the same numbers can come from a pure-CPU 24 ms frame
    paced to two vblanks.
  - `notes_name_each_step_and_failed_trial_once` (one note per step, one per failed trial,
    9-10 summaries in 10 CPU-bound minutes).
  `cpu_bound_frames_keep_the_picture` stays the pure-CPU case, with wait 2 ms. Its last phase
  (light work, wait 32) runs 60 frames instead of 120: the wait reaches the window's p95 after a
  few frames, so the first step now comes early and the next still waits its own window.

## Measurements (littlemallard, headless pipelined, 3840x2160, 4 views, AI seats, elite, seed 7, Team Size 200%, 4 think threads, 60 s after 1500 warm-up ticks, ES-DE idle)

| row | frame p50/p95/p99 ms | main CPU p50/p95 | wait p50/p95 | GPU p50 | within 33.3 | governor |
|---|---|---|---|---|---|---|
| kas2 XL no UI, scale 1.0 fixed | 29.7/33.4/35.0 | 28.9/32.7 | 0.00/0.00 | 18.7 | 94% | - |
| kas2 XL no UI, 0.85 fixed | 28.0/31.9/33.2 | 27.3/31.2 | 0.00/0.00 | 16.8 | 99% | - |
| kas2 XL no UI, 0.5 fixed | 25.9/29.8/31.0 | 25.2/29.1 | 0.00/0.00 | 14.9 | 100% | - |
| kas2 XL no UI, governor live | 29.7/33.7/35.4 | 29.0/32.8 | 0.00/0.00 | 18.7 | 93% | held 1.0, no change |
| kas2 XL UI, governor live | 31.5/35.3/37.3 | 30.5/34.2 | 0.00/0.00 | 19.8 | 78% | held 1.0, no change |
| mus1 UI, 1.0 fixed | 19.3/23.6/25.3 | 14.6/17.6 | 5.38/8.87 | 18.4 | - (16.7: 14%) | - |
| mus1 UI, governor live | 15.5/19.9/22.0 | 13.4/17.1 | 1.39/5.98 | 14.3 | - (16.7: 69%) | 1.0 -> 0.85 at 7 s -> 0.70 at 54 s |

mus1 per scale in the governed run: 1.0 p95 23.0 (368 frames), 0.85 p95 18.8 (3059), 0.70 p95
17.5 (443). The bench logged:
`quality: map assets_baked/bf2_mus1 views 4 scale 0.85 tier -0 steps 1 cpu_bound false p95 23.0 busy p95 17.7 wait p95 8.1`,
then `scale 0.70 ... p95 20.3 busy p95 16.5 wait p95 6.8`. (Those runs predate the shared
label; the bench now prints `map bf2_mus1`, and `bf2_kas2 xl` with `--xl`, as the window does.)

No before row was measured: the bench had no governor before this packet, and the old rule was
not run in it.

Reasoned, not measured: in a split window the old governor saw `acquire_ms` of about 0
(overwritten by views 1-3), so `acquire_seen` never set and `busy` was `None`, and it stepped
down on every slow window whatever the cause. The night's black boxes show acquire 0.003-0.016
ms (perf-measure.md), but those were recorded with the views-overwrite bug, so they say nothing
about the acquire wait the new rule reads.

Raw data: scratchpad p1/lm (JSON and logs). /tmp/openbf-bench on littlemallard was deleted
afterwards; nothing under ports/openbf was touched.

## Deviations

- **The kas2 XL acceptance is not met as written.** In the pipelined bench, kas2 XL never waits
  for a frame in flight (wait p95 under 0.01 ms; GPU 18.7 ms against 29 ms of main-thread CPU),
  so the fixed rule calls it CPU-bound and holds 1.0. Lower scales still have a lower p95
  (0.85: 31.9; 0.5: 29.8 against 33.4). The gain comes from the main thread's own CPU time,
  which falls with scale (32.7 -> 31.2 -> 29.1 ms p95), not from a GPU wait. The cause is the
  shared package power, the driver's per-pixel CPU work, or both; it was not split here. Under
  60 Hz Fifo that is the share within 33.3 ms: 94% at 1.0 against 99% at 0.85. The planned
  waited-bench figure (29.8 ms GPU at 1.0) overstated the GPU's share: pipelined, the GPU is
  19 ms. So the governor no longer drops kas2 XL to the floor for nothing, but it also does not
  take the 0.85 step that would help. Catching that needs a measured trial step on CPU-bound
  frames (step down once, keep it only if the p95 falls). That would be a behaviour change
  against `cpu_bound_frames_keep_the_picture`, so it is left for after the logs are read.
- The wait threshold is a new constant, `WAIT_SHARE` = 0.25 x budget. It is OURS and
  PROVISIONAL, set from the headless bench's two cases (CPU-bound under 0.01 ms, GPU-bound p95
  6-9 ms). That bench has no vsync, and its wait is for a frame in flight, not the window's
  acquire. **The rule is untested under Fifo.** There a CPU-bound frame over budget may be paced
  to the next vblank and wait in the acquire for the rest of it (about 33.3 ms less its CPU
  time). If so, the rule reads that wait as the GPU's, and a CPU-bound kas2 XL round steps down
  to the floor, the blur this packet set out to stop. If a GPU-limited split frame waits less
  than 4.2 ms in the acquire, the rule never sees the GPU at all. Nothing here claims kas2 XL
  keeps its scale in the game.
- The windowed kas2 XL and Mustafar round with `OPENBF_FRAME_PROFILE` and
  `OPENBF_GPU_TIMING=1` was not run. It is the deciding check for the rule: the acquire wait p95
  on both maps in four views, against `WAIT_SHARE`, before anyone relies on it.

## What is left

- Playtest build: a windowed 4-player kas2 XL and Mustafar round. Read the `quality:` lines, the
  black box's `world_scale` and the acquire wait p95 (the `WAIT_SHARE` check above).
- Candidates after those logs: a measured trial step for CPU-bound frames (the kas2 XL case
  above), and remembering a failed step-up scale per (map, views) for the round (Mustafar: in
  the bench it held 0.85 with p95 18.8 and then took one more step to 0.70).
