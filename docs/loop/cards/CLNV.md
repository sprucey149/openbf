---
id: CLNV
title: Cleanse 15 (core): waivers for the two files over the strict size cap, CRATES.md counts, and whatever the review of the core diff 177140d..0487827 finds
milestone: M0
value: 3
phase: cleanup
status: done
deps:
files: engine/CRATES.md,scripts/size_waivers.txt,scripts/loopctl.py,engine/crates/core/src/ai.rs,engine/crates/core/src/ai/arsenal.rs,engine/crates/core/src/commander.rs,engine/crates/core/src/skill.rs,engine/crates/core/src/unit.rs
declares:
needs:
effort:
branch: clnv
resume: landed 72e6d623 2026-10-13
created: 2026-10-13
owner: CLEANSE 1499707
---

## Why
Cleanse 15, area core (diff 177140d..0487827: 32 files, 4,747 added lines). Findings fixed here, each VERIFIED by running or reading it:
1. `size_report.sh --strict` exits 1 on main: commander.rs (2,508) and ai/behave.rs (2,314) are over the 2,000 cap with no waiver; behave.rs had no split card (now SPL13).
2. loopctl's planner trigger `fid_blocked` fires for a card that waits on the maintainer (the planner's own ask, STATUS 2026-10-13 07:14).
3. skill.rs's tier table says the error cap is 1 m for Easy / Normal / Hard; the code gives 1.83 / 1.36 / 1.2 m. arsenal.rs calls 12 m "twice" 6.5 m.
4. unit.rs's "every field is hashed" test edits 5 of the 12 hashed floats.
5. Ten port-own constants say OURS without the ledger's keyword PROVISIONAL.
6. CRATES.md: 21 stale line counts.
