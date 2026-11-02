---
id: LRES
title: loopctl pick hands out cards it should not: (1) a cleanse's CLN* card while the cleanse session that opened it is live (it holds CLEANSE, not the card id); (2) any parked card as a land job, even with no branch and unmet deps (SQVEH, 2026-09-30 21:36 - the prompt now parks vehicle cards that lack a runnable check)
milestone: M0
value: 5
phase: build
status: done
deps:
files: scripts/loopctl.py
declares:
needs:
effort:
branch: lres
resume: landed 0c96beed 2026-10-02
created: 2026-09-30
alpha: yes
---
## Why
pick() step 1 resumes any in-progress card whose id is not a live claim. A cleanse/planner/audit session claims CLEANSE/PLANNER/AUDIT and opens a CLN* (or other) card without claiming its id, so the next free lane is told to resume that card while the owner is still working it — 2026-09-30 06:50 lane 1 was handed CLNT while lane 2 (pid 9841, CLEANSE + MAIN) ran `land.py CLNT --pid 9841`. Lane 1 stood down (report reports/20260930_0652_resume_CLNT.md); a less careful session would have edited the worktree mid-gates or run a second land.py.
## Fix (suggested)
Either a session that opens a card claims its id too (`loopctl new` under a live job claim, or cleanse.md tells the cleanse to `claim <ID>`), or step 1 skips a card whose `branch` has a live land.py/gates.py, or whose id appears in lead_tmp/<branch>/land/state.json with a live pid.
