---
id: WEARX
title: SSD wear: the gates build only the examples they run (cargo build --release -p game --bins --examples links ~27 example binaries, each the whole engine; the maintainer 2026-10-15 - SSD wear is a big deal)
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
The maintainer, 2026-10-15: SSD wear is a big deal. Measured that day: the WD SN850X (the system/home drive, btrfs) took 289 GB in 3.6 h of uptime, 245 GB of it from openbf.slice (cgroup io.stat); SMART lifetime 17.5 TB written, 1 % used, 560 power-on hours, so the loop writes ~78 GB/h against a lifetime ~31 GB/h. Almost all of it is cargo output: engine/target is 154 GB (debug 121: deps 71, examples 31, incremental 19; release 33: examples 27), and each lane builds its own target (lead_tmp/*_target 120-175 GB apparent each).
Already done (commit 46c6555): [profile.release] and [profile.dev] debug = line-tables-only, dev incremental = false.

## Evidence
see Why; /proc and io.stat numbers in the chat of 2026-10-15.
