---
id: BLKB
title: Playtest black box (dev mode): the windowed game keeps the last 60 s of every unit's state in memory and writes a snapshot file on F9, on exit and on a panic, so the maintainer's bug reports come with the bots' goals, legs, steer picks and positions, and the maintainer's own state
milestone: M0
value: 5
phase: build
status: done
deps:
files: engine/crates/game/src/blackbox.rs,engine/crates/game/src/unit_probe.rs,engine/crates/game/src/main.rs,engine/crates/game/src/main/windowed.rs,engine/crates/game/src/assets.rs,engine/crates/game/src/lib.rs,engine/crates/core/src/ai.rs,engine/crates/core/src/ai/steer.rs,engine/crates/core/src/collision/walls.rs,engine/crates/game/examples/blackbox_read.rs,engine/crates/game/examples/round_to_outcome.rs,engine/CRATES.md,CLAUDE.md
declares:
needs:
effort:
branch: blkb
resume: landed 7b64fcd2 2026-10-02
created: 2026-10-02
alpha: yes
first: 1
---

## Why
The maintainer, 2026-10-02: he now plays the port himself and each time comes back with screenshots and a description. A snapshot of
the game's own state (every bot's task, position, route leg, steer pick; the maintainer's own position and input) lets Claude find
the cause from the file instead of guessing from a picture. DEV ONLY: it goes away (off by default, compiled in but
inert) when the game is playable for real. The maintainer's flow: he plays, sees something wrong, presses F9 (or just closes the
game), comes to Claude with a screenshot and a line; Claude reads the snapshot, writes the finding into a card for the
loop, the maintainer restarts the loop and plays again. The loop's own sessions may also use it (a headless run can write one).

## Stretch (a second card if this one lands cleanly - do not hold this card for it)
**Input recording + replay:** record the human's input stream and the RNG seed from the start of the round into the
snapshot; `blackbox_read --replay <tick>` runs the sim headlessly to that tick and drops into the usual probes. This only
works if a round with a human in it is bit-deterministic: first measure that (record a 5-minute round, replay it, compare
the sim hash every second) and card the result either way.

## Notes
- Related: SQMK (the dev overlay, `OPENBF_DEV`), OBST (the first card this tool should serve: snapshot the maintainer's
  wall-walking bots), the stuck census (`STUCK_CENSUS`), `UNIT_SAMPLES` in `round_to_outcome` (its per-unit second samples
  hold most of the bot fields already: reuse the field list and the code that builds them; do not write a second one).
- Run any new probe or example binary under the memory cap (INBOX 2026-10-01 22:00).
- It is DEV scaffolding: keep it in its own module (`game/src/blackbox.rs`) so removing it later is one delete.

## Built (2026-10-02, lane 2) - what landed and the checks that were run
`game/src/blackbox.rs` (the ring, the file, the triggers), `game/src/unit_probe.rs` (the unit row, shared with the sweep's
`UNIT_SAMPLES`), `examples/blackbox_read.rs`, `round_to_outcome`'s `BLACKBOX_AT` / `BLACKBOX_ON` / `BLACKBOX_TIMING`,
two read-only core diagnostics (`CollisionWorld::wall_sweep_body`, `ai::way_ahead`), root `CLAUDE.md`, the line in
`briefs/LOOP_PROMPT.md` (and the archived `briefs/v1/OPUS_LOOP_PROMPT.md`). `engine/artifacts/` was already gitignored.
Acceptance, each RUN (evidence `engine/artifacts/lead_tmp/blkb/`):
- Windowed, dev mode, own Xvfb display (`win_run.sh`: WAYLAND_DISPLAY unset, private runtime dir, scratch profile, no
  sound device; F9 by a raw-X11 XTEST client `xkey.py`): BF2 KAM1 16 v 16, F9 twice then the window closed -> `last.json`
  (exit), `prev1.json`, `prev2.json` (hotkey), and `blackbox_read` prints the digest (`win1/`). A forced panic
  (`OPENBF_BLACKBOX_PANIC=450`) wrote `last.json` with reason `panic: ... forced at tick 450`, exit 101 (`win2/`).
  kill -SEGV is NOT covered (no signal handler; a Rust panic is).
- Overhead, BF2 KAM1, 32 units, 9000 ticks headless (`timing/`): the recorder costs mean 63 us per tick, p99 1.0 ms, max
  1.5 ms (the 0.5 s unit frame); the hotkey's share of a frame is a 0.13 ms copy, the encode + write 4.1 ms on its own
  thread. Windowed with 65 units: frame mean 19.1 / p99 21.8 / max 22.0 ms before the F9 write, 19.1 / 21.8 / 22.1 ms
  after it (no hitch). Ring memory 1.19 MB at 32 units, 2.00 MB at 65.
- File size, 65 units x 60 s (BF2 KAM1 `--ai 32 --ai-cap 64`): 4.6 MB (under 8 MB).
- The recorder only reads: the sweep's stdout with the black box on is byte-identical to off (result + hash, 3 maps at
  3000 ticks and KAM1 at 9000), the `UNIT_SAMPLES` CSVs byte-identical to the pre-change binary's on 3 maps; land.py's
  gates below.
- Tests (`game --lib blackbox`): the wrap at 60 s, the header + read-back, the atomic rotating write, off = no ring and
  no folder, the stuck alarm. Mutation-checked: a wheel that does not advance fails the wrap test.
Cut / different from the card: no `last.png` (the windowed renderer cannot read its frame back); the stuck alarm wants
the stick PUSHED for 60 % of the 20 s (standing still on purpose is not a bug, and an alarm rotates the files); a headless
snapshot goes to `playtest/headless/<map>_s<seed>.json`, never over the maintainer's `last.json`; the "wall / barrier / ramp /
steep hit" is asked by the probe along the straight line to the leg's aim point at each 0.5 s sample, not carried by the
brain (core's state and hash are untouched). Stretch (input recording + replay): not started - a second card if wanted.
