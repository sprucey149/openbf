# PERF-0 result: a splitscreen bench that runs frames as the window does (2026-11-27)

## What changed

- 0a (`Bench splitscreen frames view by view`, as committed on `claude/perf-bench`):
  `--bench-views`, `--bench-scale`, `--xl`, `--team-size`, `--capture-frame`;
  `ReplayFrame::build_view`; `Renderer::time_every_view`.
- 0b (detailed bench, `OPENBF_BENCH_DETAIL`):
  - `--bench-pipelined [--bench-seconds S]`: frames as the window runs them. Ticks run on the
    wall clock through the walk's accumulator and its cap, so a slow frame runs two ticks.
    Nothing waits for the GPU after a view, and at most two frames are in flight. The report
    gives frame p50/p95/p99, main-thread on-CPU per frame (CLOCK_THREAD_CPUTIME_ID), tick, UI,
    draw, wait, GPU, and the share of frames within 16.7 and 33.3 ms. It also gives a sim hash
    every 30 ticks.
  - `sensors.rs` samples at 4 Hz on its own thread: scaling_cur_freq (average, min and max), amdgpu
    hwmon sclk, power and busy %, the `gpu_metrics` v2 table (socket, soc and gfx W, gfxclk,
    uclk, fclk, per-core clocks, throttle bits), and k10temp Tctl. It also follows the bench's
    main thread: the CPU it last ran on (`/proc/self/task/<tid>/stat`) and that CPU's clock
    (`main_cpu_mhz`), since the busiest core need not be the main thread's.
  - The summary gives `seat_ms` (the seats' own `ai::think`, main-thread work inside `cpu_ms`
    that a window with people at the pads does not do) and `main_cpu_mhz`.
  - The detailed-bench-only flags (`--bench-views`, `--bench-pipelined`, `--bench-seconds`,
    `--bench-seats`) are refused without `--bench` and `OPENBF_BENCH_DETAIL`, and
    `--capture-frame` is refused when the detailed bench would draw nothing.
    `OPENBF_BENCH_PLAYER=script` (the one-player gate's scripted human) is refused with any
    `--bench-seats` other than `idle`, so one pad never has two drivers. It stays a separate
    switch because the gate's pinned hashes depend on it running without the seats' spawn
    screens.
  - `--bench-seats idle|script|ai`: with `ai`, the bots' own `ai::think` drives each local
    seat, with its state kept in the bench and never in the sim. The seats spawn through their
    own spawn screens, as in the window. Each seat's kills, deaths, lives and metres walked are
    reported.
  - With `--hud` and more than one view, the bench draws through the window's own
    `split::render_views`. That covers SplitUi HUDs with minimaps, name tags
    (`name_tags::cell_tags`), each seat's event feed, the spawn-preview soldiers, and the egui
    text in each cell. Captures on littlemallard: kas2 XL, 4 views, at the end of the 60 s UI
    row (all four HUDs, minimaps, a name tag, the crowds); and kas2 XL at the round's start with
    every seat on its spawn screen (command-post pages and minimaps in all four cells; the
    spawn-preview soldiers of views 1-3 at the foot of their cells; none visible in view 0's
    cell). View 0's preview in the bench was not compared against the window.
  - The waited path (`--bench-frames`) also records `present_gpu_ms`, the timer's own sum over a
    present, as a check on the per-view sum. On kas2 XL with 4 views it was 29.41 against
    29.41 ms.
- 0c: the `GpuTimer` sums every view of a present under `time_every_view` (`gpu_sections`) and
  keeps the newest single view (`gpu_view_sections`). It publishes a sum only when every view
  of the present was timed. That bookkeeping is a pure `PresentSums` with unit tests (a full
  4-view present, one missing its first, a middle or its last view, and a single view equal to
  the old `last`). The ring grew from 3 to 12. The window sets `time_every_view` only
  when the GPU timer exists (`OPENBF_GPU_TIMING=1`). Without it nothing changes, and one view is
  its own present's first and last view.

## Tests

- `scripts/one_player_gate.py`: every row matches (lan_soak ok, 1559 ticks, 4312 hash checks).
- `cargo test -p game --bin game a_format_two_table`: 1 passed.
- `cargo test -p render --lib split`: 3 passed.
- `cargo test -p render --lib gpu_timer`: 2 passed. `cargo test -p game --bin game sensors`:
  2 passed.
- Sim hashes across thread counts on kas2 XL with 4 views and AI seats: 4, 6 and 8 think
  threads, the noui and scale rows, and the GPU-free `simonly` run all agree at every common
  30-tick checkpoint (59-60 per row, up to tick 3270). hot1 pipelined against hot1 simonly
  agrees at 60 checkpoints.
- Not run: windowed `OPENBF_GPU_TIMING=1` with split views (no window was allowed on
  littlemallard). That code path is only the flag and the timer's summing, which the bench
  exercises. The split_capture tests were not rerun, because the change only touches the timer
  and visibility.

## Measurements (littlemallard, headless, 3840x2160, elite, seed 7, Team Size 200%, ES-DE idle)

Before is the committed bench (waits after each view, idle seats, no UI). After is pipelined,
with AI seats and split UI, starting mid-round after 1500 (kas2) or 1800 (hot1) warm-up ticks.
The night's frames covered ticks 1545-3330 on kas2 and 1860-3645 on hot1. A 60 s run covers
ticks 1530-3330, the night's span. The 90 s rows ran on to tick 4230 (kas2) and 4530 (hot1),
past the night: their frame and CPU columns below are restated over the first 60 s, and the
last 30 s is its own row. By tick 3300 the kas2 scores stood at 337 and 325 of 350, so that
late stretch is the round's end.

| row | frame p50/p95/p99 ms | main CPU p50/p95 | tick (step_world) p50/p95 | draw (lap) p50 | GPU p50 | socket W | busy core MHz | sclk |
|---|---|---|---|---|---|---|---|---|
| kas2 XL 4v before (waited) | - | ~24 est. | 6.78/8.29 sim only, update 8.64 | build 4.63 + submit 8.75 = 13.4 | 29.4 (summed views) | - | - | - |
| kas2 XL 4v pipelined, UI, 4 threads, first 60 s of 90 | 31.3/35.2/36.5 | 30.1/33.9 | 10.3/11.7 | 20.3 | 19.7 | 33 | 4760 | 2000 |
| same, rerun at 60 s (review) | 31.2/35.2/37.0 | 30.3/34.3 | 10.3/11.8 | 20.2 | 19.6 | 34 | 4760 (main 4759) | - |
| same, last 30 s of the 90 s run (ticks 3330-4230, round's end) | 34.2/40.1/47.2 | 33.1/38.6 | 10.3/11.9 | 22.9 | 22.1 | - | - | - |
| same, 6 threads, 60 s | 31.0/34.7/36.2 | 29.9/33.5 | 9.9/11.3 | 20.4 | 19.5 | 35 | 4740 | 2210 |
| same, 8 threads, 60 s | 30.8/34.5/36.0 | 29.8/33.5 | 9.8/11.2 | 20.4 | 19.4 | 35 | 4740 | 2240 |
| kas2 XL 4v pipelined, no UI, scale 1.0 (review repeat) | 29.3/33.4/34.7 (29.5/33.3/34.5) | 28.6/32.6 | 10.39 (10.44)/11.8 | 19.0 | 18.6 | 37 | 4710 (main mean 4567) | 2435 |
| same, scale 0.5 | 26.0/29.9/31.2 | 25.2/29.1 | 9.8/11.1 | 16.6 | 14.9 | 24 | 4760 | 1000 |
| same, scale 0.25 (review repeat) | 25.3/29.3/30.6 (25.5/29.6/30.9) | 24.4/28.4 | 9.67 (9.71)/11.0 | 16.0 | 12.7 | 22 | 4760 (main mean 4662) | 800 |
| kas2 XL 1 view, HUD (one seat: a different sim) | 14.4/21.9/23.4 | 8.1/21.1 | 12.0/13.3 | 7.1 | 13.1 | 52 | 4700 | 2600 |
| kas2 XL sim only, no GPU (ticks 1500-3330) | - | - | 6.74/8.63 sim only | - | - | - | - | - |
| hot1 XL 4v pipelined, UI, first 60 s of 90 | 24.0/28.5/30.3 | 23.1/27.6 | 6.6/8.1 | 17.6 | 15.2 | 38 | 4710 | 2520 |
| hot1 XL 4v pipelined, no UI, scale 0.25 | 18.8/23.4/25.2 | 18.2/22.6 | 6.0/7.4 | 14.1 | 10.1 | 22 | 4760 | 800 |
| hot1 XL sim only (ticks 1860-3645) | - | - | 4.02/5.22 sim only | - | - | - | - | - |
| mus1 4v pipelined, UI | 18.6/22.6/24.2 | 14.3/17.6 | 2.3/2.8 | 11.9 | 17.7 | 47 | 4720 | 2600 |

Over the night's span (ticks 1530-3330), kas2 XL 4v with UI: 80% of frames within 33.3 ms
(80% again in the rerun), under 1% within 16.7 ms. Two-tick frames: 8 of 1965 (10 of 1969 in
the rerun), main CPU 42.8 ms (44.1), ticks 20.7 ms, draw 22.7 ms. The 90 s run's earlier
figures (67% within 33.3, 51 two-tick frames at 43.5 ms) were mostly its last 30 s: 43 of the
51 two-tick frames fall there, and only 35% of that stretch's frames are within 33.3.
`seat_ms` (the seats' own `ai::think`) is 0.13-0.14 ms p50 and 0.27-0.31 ms p95 per frame in
every 4-view row, so the AI seats add well under 1% to `cpu_ms`. uclk and fclk stayed at
2800 MHz in every row. Tctl was 62-71 C. The gpu_metrics throttle word showed bits 0x2, 0x800
and 0x1000 on and off in every row, including light ones. These bits are not decoded here. The
scaling_cur_freq average (about 2.2 GHz) includes idle cores. The review rows follow the main
thread's own core: it moved over 13-14 CPUs in each run, and its clock was p50 4.72-4.76 GHz,
mean 4.57 GHz at scale 1.0 without UI against 4.66 GHz at scale 0.25 (10% against 4.6% of
samples under 4.5 GHz).

Not in the bench: the window's render blend. Headless runs keep it off, so the per-tick pose
capture (`Blend::before_tick` in `WalkSim::advance_while`) and the per-view blend in and out are
missing from these figures. Audio and input polling are missing too.

## The night-vs-bench gap (kas2 XL, the night's slow-frame median, main thread on-CPU)

Night: 55.4 ms on-CPU, made of sim 23.9 (two ticks), update 2.5, draws 3.1 and render lap 26.5.
The old bench estimate was about 24 ms. The items below rebuild the new bench's figure from the
old one. They do not rebuild the night: about 12 ms of it is left over (last item).

- **A second tick per frame: about +12 ms.** The night's slow frames run two ticks. Over the
  night's span the bench's two-tick frames take 42.8 ms on-CPU (44.1 in the rerun), against
  30.4 ms for a one-tick frame. A frame over 33 ms runs two ticks, and the next frame is slower
  again. In the bench this loop stays rare over the night's span (8-10 frames in 60 s).
- **Per-view CPU work: about +5 ms over the old bench.** The split UI (HUDs, tags, minimaps,
  spawn previews) adds 2.2 ms of draw and 0.2 ms of UI models. The window's render path,
  mid-round, against the waited path's build plus submit adds about 2.6 ms (16.0 against 13.4,
  with the GPU nearly idle at scale 0.25).
- **GPU load: about +3.7 ms of CPU per frame at scale 1.0, mostly not from clocks.** With
  identical CPU work and the same hashes, cutting the GPU load (scale 1.0 to 0.25, socket 37 W
  to 22 W) takes the tick from 10.4 to 9.7 ms and the draw from 19.0 to 16.0 ms. Repeating both
  rows gave ticks within 0.05 ms of the first runs (10.39 and 10.44, 9.67 and 9.71), so the
  0.7 ms tick difference is well outside run-to-run noise. The 4-, 6- and 8-thread rows differ
  by 0.4-0.5 ms, but by design (thread count), so they are not a noise band. The main thread's
  own core averaged 2% slower at scale 1.0 (4.57 against 4.66 GHz), which accounts for about
  0.2 ms of the tick and 0.4 ms of the draw difference. The rest (about 0.5 ms of tick and
  2.6 ms of draw) is not frequency on the main thread's core, and memory clocks do not move.
  That rest fits shared memory and cache contention with the iGPU. It also fits a package power
  or bandwidth effect, which these rows cannot separate. The 1-view row (tick 12.0 ms at 52 W
  socket, the highest of all) does not settle it either way: with one seat it runs a different
  sim (a different hash), and its main-core clock was not recorded. The sim alone, with no GPU,
  is 6.7 ms per tick mid-round. In step_world under load it is 9.7-10.4 ms, about 1.9 ms of
  which is the frame's own per-tick extras (stations, fx, bolts, feeds).
- **Left over, unexplained: about 12 ms, 22% of the night's slow frame.** On like-for-like
  two-tick frames the night is 55.4 ms on-CPU and the bench 42.8 ms. The difference is sim 23.9
  against 20.7 (+3.2), render lap 26.5 against 22.7 (+3.8), and 5.6 ms of update and draws on
  the night that the bench does not have (its UI models are 0.2 ms, its seats 0.1 ms). Candidate
  causes, none measured: the render blend, off in the bench (per-tick capture and per-view
  blend in and out); audio and input; and the real fight (the night's players fought more than
  the AI seats: 2 kills and 1 death over 90 s for seat 1). Round growth is not one: the
  sim-only tick is 7.5 ms over ticks 1-900 and 6.7 ms over ticks 1500-3330 on kas2. Until this
  is explained, the bench under-reads the night by about 12 ms on two-tick frames.

hot1 XL: the night was 34.7 ms on-CPU for one-tick slow frames, with a median frame of 27.6 ms.
Over the night's span the bench's one-tick frames give 24.2/28.1 ms (p50/p95) on-CPU, and the
median frame is 24.0 ms. The bench's p95 one-tick frame is still 6.6 ms under the night's
slow-frame median. GPU load again adds about 0.7 ms to the tick and about 4 ms to the draw (UI
included).

## Decisions this feeds

- **PERF-5 gate: not shown by this bench; the call is the lead's.** The plan's aim for kas2 XL
  was p50 at or under 33.3 ms, and its gate asks whether per-view CPU work keeps kas2 XL above
  that. Over the night's span the bench's p50 is 31.3 ms (31.2 in the rerun), so on the plan's
  own p50 reading the bench already meets the aim before PERF-5. An earlier draft of this note
  declared the gate met by reading it at p95 instead, without saying so. At p95 the bench is
  35.2 ms, 1.9 ms over the step, with 20% of frames over 33.3 ms. The render lap is 20 of the
  30 ms main thread, and the main thread's core loses about 2% to clocks. The night, though, ran
  about 35 ms on average, and the bench under-reads it by about 12 ms on two-tick frames (see
  the leftover above). Two readings follow:
  - If the gate is read at p95 on the bench (a proposed change from the plan's p50, because the
    vsync step is crossed by the slow fifth of frames, not the median): PERF-5 needs at least
    2 ms off the lap, measured as pipelined p95 over ticks 1530-3330. Aim for 3 ms.
  - Against the night: 55.4 ms two-tick frames need about 22 ms off to reach 33.3, far beyond
    PERF-5. Most of that gap is the second tick and the unexplained 12 ms, not per-view work.
  The windowed frame profile on the playtest build (PERF-1) should explain the 12 ms before
  PERF-5's scope is fixed.
- **Think threads: AUTO_CAP stays 4.** Over the same tick span, 6 threads save 0.39 ms per tick
  and 8 save 0.50 ms (median), below PERF-2's 1 ms bar. Over the same 60 s, frame p95 is 35.2
  (4 threads), 34.7 (6) and 34.5 (8) ms, and two-tick frames 8, 7 and 5. Main-thread draw did
  not change (20.3 against 20.4 ms). Hashes were identical.
- **Sim off the main thread: comes back as a candidate for after PERF-5, not now.** On kas2 XL the
  sim plus per-tick extras is about 10 ms of every one-tick frame and about 20 ms of every
  two-tick frame. Even with PERF-2 and PERF-5 together, the bench's main thread stays around
  25 ms. 40+ fps on XL splitscreen needs the tick overlapped with the render. This rests on the
  bench; the night's unexplained 12 ms could move it either way.
- **PERF-3/4: a partial CPU credit, a modest move up.** The main thread's core loses about 2%
  of its clock under full GPU load. GPU work still costs CPU time, mostly through contention or
  package effects rather than that clock, about 0.5 ms of main-thread CPU per GPU ms saved on
  kas2 (5.9 GPU ms gave about 3.3 ms of frame time). Mustafar is GPU-bound in the bench
  as on the night: GPU 17.7 ms, frame p95 22.6 against the 16.7 aim.
- Targets written down for the later packets (pipelined, littlemallard, same scenarios):
  - kas2 XL 4v with UI, ticks 1530-3330: p50 31.3 and p95 35.2 now. The plan's p50 aim is met
    on the bench; the proposed p95 aim is at or under 33.3 (see the gate).
  - hot1 XL 4v with UI, ticks 1830-3630: p50 24.0 and p95 28.5 now. Aim for a lower share over 20 ms, which is
    now most frames.
  - mus1 4v with UI: p95 22.6 now. Aim for 16.7 or under.

## Deviations

- The thread-count rows ran for 60 s at 6 and 8 threads and 90 s at 4 threads. Every 4-thread
  figure is restated over its first 60 s, and a 60 s rerun agrees.
- Review rows (one 60 s UI rerun, repeats of no-UI scale 1.0 and 0.25, a spawn-screen capture)
  ran on littlemallard the same way, in a fresh /tmp/openbf-bench, deleted afterwards. Their
  hashes matched the first rows at every common checkpoint.
- Bench runs read the installed openbf.pak beside a stripped binary in /tmp/openbf-bench on
  littlemallard. The directory was deleted afterwards and nothing under ports/openbf was touched.
- The `--bench-scale 1.0` in the run script's common flags was overridden by the later flag in
  the scale rows (the last value wins).
- The windowed 0c path was not exercised in a window.

## What is left

- Windowed confirmation of `OPENBF_GPU_TIMING=1` split sums on the playtest build (PERF-1's
  windowed round).
- Decoding the APU throttle bits, if PERF-3/4 want a power-limit reading.
- The 12 ms the bench does not reproduce on kas2 two-tick frames (windowed frame profile).
- The render blend in the bench, if the windowed profile points at it.
- View 0's spawn-preview soldier in the bench's spawn-screen capture.
- PERF-5 call trees on a pipelined kas2 XL 4v run (this bench is the instrument).
