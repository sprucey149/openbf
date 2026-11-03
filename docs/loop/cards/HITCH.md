---
id: HITCH
title: Hitch hunt: the windowed game freezes or dives for a moment now and then while sound carries on (the maintainer, 10-20: BF1 NAB2, GEO1, BF2 KAS2) - a frame-time recorder in dev mode first (ticks run, AI, render CPU, present, GPU objects made per frame; frames over 25 ms into the black box), then the causes the numbers name, fixed at the root (load-time uploads, no GPU objects made in the frame, AI bursts spread)
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
The maintainer, 2026-10-20 playtest (INBOX 16:50 + his 17:05 standing word): the windowed game hitches - the picture freezes or dives
for a moment now and then while the sound carries on (BF1 Naboo 2 and Geonosis, BF2 KAS2; most of the time fine). Sound
has its own thread, so the main thread blocks. "The game on modern hardware (this laptop) must run with no hitches at all."
Root causes first, threads last: fix the structure (load-time uploads, per-frame allocation, AI bursts, O(N^2) queries);
never use threads to hide a slow path. Measure first, fix second; not held behind PERF.

## Evidence (where to start)
- `game/src/frame_profile.rs` (card ISPD): `OPENBF_FRAME_PROFILE=<csv>` already writes per frame the sim ticks, updates,
  draw lists / HUD, `render_frame` and the image acquire; `OPENBF_GPU_TIMING=1` adds GPU ms per section. Extend it, do not
  write a second recorder.
- `game/src/walk.rs:804` `advance_with` + `core::MAX_FRAME_ACCUM_SECS` 0.1 (core/src/lib.rs:59): one slow frame makes the
  next run up to 3 ticks.
- First-use GPU work: `viewer::scene::get_texture` (textures uploaded on first use), `viewer/vehicles.rs:319 pose_meshes`
  (walker / skinned meshes made on first use); ~46 `create_buffer|texture|bind_group|render_pipeline` sites in render + game.
- AI bursts: many bots re-planning or recycling on one tick; the black box's own ring and writes (`game/src/blackbox.rs`).

## Part 1 - the recorder (dev mode, ON by default like the black box; `OPENBF_DEV=0` off)
Per frame: ms of the sim ticks and HOW MANY ran, the AI part of them, render CPU, present / GPU wait, and every GPU resource
made that frame (texture, buffer, mesh, bind group, pipeline; name and bytes). A frame over 25 ms is kept with its breakdown
in the black box snapshot (F9, quit, panic), plus a one-line summary (frames, p50 / p99 / worst ms, hitches per minute, what
the worst five were spent on); `blackbox_read` prints both.
## Part 2 - the suspects, confirmed or cleared with numbers
Textures / meshes on first use, the 3-tick catch-up, AI bursts, per-frame buffers, the black box, lazy pipelines. Replay
BF1 NAB2, GEO1 and BF2 KAS2 headless-windowed (isolated display, `loop/gpu.lock`) with the camera / player moving over the
map, 3 min each, on this machine's RTX and its Intel iGPU.
## Part 3 - the fix the numbers choose
Likely: make everything a map can show at load time behind the loading screen; spread an AI burst over ticks; stop creating
GPU objects inside the frame. One commit per cause, each with its before / after.

## Results (2026-10-20, branch hitch 3f1b36c; report `reports/20261020_2000_card_HITCH.md`)
Part 1 built: the frame record (dev mode; `OPENBF_FRAME_RECORD=0` off) in `game/src/frame_profile.rs`, the black box's
`hitches` (schema 6), `blackbox_read` FRAMES / `--frames`, the summary line at the battle's end. Per frame: phases,
ticks run + AI ms + hook ms, image wait, GPU ms, on-CPU / run-queue ms, files read, every GPU object made (render::made).
Part 2, the suspects (RTX, 4K, 185 s autopilot each, `lead_tmp/hitch/win/`):
- first-use loads: CONFIRMED and fixed (effect / bolt sheets, pickup models, sound takes: 0.1-5.7 ms each in a frame).
  Frames that made GPU objects past 5 s, base -> fix: NAB2 111 -> 12, GEO1 68 -> 4, KAS2 79 -> 1.
- AI bursts: CLEARED as a code cause. Single thinks of 4-31 ms appear, but the same seed run twice alone gave 32 and 7
  of them at different ticks (`lead_tmp/hitch/think_solo_{a,b}.txt`): the machine, not a search.
- 3-tick catch-up, per-frame buffers, the black box, lazy pipelines: CLEARED (hook p99 0.5-0.8 ms; no pipeline or
  per-frame buffer made in battle; 2+ ticks only after a slow frame).
- the machine's other work: CONFIRMED as what freezes frames (40 loop-like rounds in the same cgroup: NAB2 1,307 over
  25 ms, worst 62 ms; the fix run's 72 / 67 ms frames were 55 / 51 ms queued) -> card HITCH2.
- GPU neighbour (a second game at 733 fps on the RTX): little (40 over 25 ms, worst 27.3 ms).
Acceptance (vsync, frames over 25 ms past 5 s, base -> fix): NAB2 105 -> 142 (loaded machine, runnable max 35),
GEO1 24 -> 10, KAS2 7 -> 4; worst 73.4 -> 71.5, 31.2 -> 25.8, 26.2 -> 25.5. NOT the asked 0: every frame left on the
quiet maps is a 24-25 ms wait for the window's image with 1-2 ms on the CPU and 0 queued (the private kwin's release),
NAB2's are the machine. Without vsync on a quiet machine NAB2 had 0 (worst 21.4 ms). iGPU NAB2 1080p: 0 / min.

## Notes
- Card RINT (render interpolation, approved by the maintainer 10-20) waits on Part 1 of this card: the recorder shows the frame split.
- Threading is PERF Part 3 (after Part 2), automatic worker count, identical sim for any count (the maintainer 10-20 (c)).
## Landed 2026-10-20 21:37 (9e381b1, lane 2; report `reports/20261020_2138_land_HITCH.md`)
Reproduced on the landed binary: KAS2 4 over 25 ms / 1 GPU-object frame (as built); GEO1 GPU-object frames 67 -> 3. GEO1's
over-25 count read 15 to 307 with the same binary as the machine's load moved (an audit sweep): HITCH2's subject.
