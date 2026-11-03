---
id: TPOSA2
title: TPOSA's Mygeeto build-time test is flaky under ci's parallel load (0.374 s wall-clock against 0.056 s alone): measure the thread's CPU time (/proc/thread-self/stat)
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
TPOSA (85834ec) added mygeetos_position_table_is_built_in_under_a_third_of_a_second, a wall-clock bound. In the TPOS branch's ci (load 25, nextest running tests side by side) it read 0.374 s and failed; alone and in round_to_outcome the build is 0.056 s. The test is on main: every lane's ci can fail on it.
