---
id: HITCH2
title: the maintainer's hitches while the loop runs: name the cause from his next F9 record (queued / files columns), reproduce the governor's polite mode (loop on the E-cores, game on the P-cores), then make play free of the loop's interference
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
Card HITCH (2026-10-20) found the game's own work under 25 ms a frame on the RTX (4K, no vsync, quiet machine: BF1 NAB2
worst 21.4 ms) and moved every in-battle load to the battle's load. What freezes frames is the machine's OTHER work:
40 loop-like headless rounds in the game's own cgroup made NAB2 1,307 frames over 25 ms in 185 s (worst 62 ms, against
86 / 28.4 ms quiet), and in HITCH's after-run the two worst frames (72 / 67 ms) were 55 / 51 ms runnable but not run
(the record's `queued`) while 35 tasks were runnable. The maintainer's 10-20 playtest fell in the governor's POLITE mode
(`loop/cpu_governor.log`: 16:24-17:03): the loop on the E-cores 16-31 at CPUWeight 10, his game free on the P-cores -
so what reached his game there is still open: the shared L3 / memory bandwidth / package power, the disk, or lane 2's
GPU gates (SSELB landed at 16:41; a GPU-saturating neighbour alone cost little: 40 frames over 25 ms, worst 27.3 ms).

## Evidence
- `engine/artifacts/lead_tmp/hitch/win/`: `bf1_nab2_load40` (the load run), `*_ab_base` / `*_ab_fix` (HITCH's A/B),
  `hog_a` (the GPU neighbour), each with `load.log` (time, 1-min load, runnable now) and `bb/last.json` (the record).
- `engine/artifacts/lead_tmp/hitch/tools/`: krun.sh, load.sh, gpuhog.sh, drops.py, dist.py, phases.py.
- A loop session cannot place a test game as the maintainer's runs: the `scripts/loopbin/systemd-run` shim puts scopes in
  openbf.slice and `scripts/loop_cpu.py` pins every thread of it (E-cores in polite mode) - HITCH did not route around it.

## Acceptance
1. The cause of the maintainer's next hitch named from his own F9 / quit snapshot (FRAMES: `queued`, `files`, `made`, the phases).
2. A polite-mode run (game on the P-cores at weight 100, loop-like load on the E-cores at weight 10) with its frames over
   25 ms counted against the quiet machine's (needs the maintainer's go for a scope outside openbf.slice, or loop_cpu.py forced).
3. If the loop is the cause: a fix that brings (2) to the quiet machine's count. Proposal: the dev game writes
   `engine/artifacts/loop/PLAYING` while a battle is open and loop_cpu.py freezes openbf.slice's heavy scopes
   (`systemctl --user freeze`) or holds them at weight 1 on the E-cores until it is gone.
