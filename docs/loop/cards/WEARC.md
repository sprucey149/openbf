---
id: WEARC
title: SSD wear, ci side (split from WEARX): measure the bytes one ci.sh run writes, then stop ci from linking example binaries it never runs (nextest / cargo test build every game example in debug; clippy --all-targets only checks them) - the maintainer 2026-10-15: SSD wear is a big deal
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
The maintainer, 2026-10-15 (card WEARX, his own session): the loop writes ~78 GB/h to the system SSD, almost all cargo output. WEARX's step 2 (gates.py builds every release example) edits a guard file and waits on the maintainer's approval; its step 3 - ci.sh - does not, so it is split off here (planner 2026-10-15 23:45).
