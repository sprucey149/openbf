---
id: TPOSA2
title: TPOSA's Mygeeto build-time test is flaky under ci's parallel load (0.374 s wall-clock against 0.056 s alone): measure the thread's CPU time (/proc/thread-self/stat)
milestone: M0
value: 5
phase: build
status: done
deps:
files: engine/crates/game/tests/nav_walls.rs
declares:
needs:
effort:
branch: tposa2
resume: landed 9443f7e0 2026-10-16
created: 2026-10-16
owner: TPOS 8632
---
## Why
TPOSA (85834ec) added mygeetos_position_table_is_built_in_under_a_third_of_a_second, a wall-clock bound. In the TPOS branch's ci (load 25, nextest running tests side by side) it read 0.374 s and failed; alone and in round_to_outcome the build is 0.056 s. The test is on main: every lane's ci can fail on it.
