---
id: LOOPT
title: Loop tooling for today's stalls: (1) the land picker skips a parked card with needs: set, as the resume step does; (2) gates.py / land.py stop at once with ADAPTER LOST when the adapter the goldens were rendered on is absent, instead of a run that moves every golden; (3) loopctl.py worktrees [--yes], the reviewed scratch-worktree sweep the maintainer approved (STATUS 49, INBOX 2026-10-15); (4) lane_alive() takes a reused THREAD id for a live lane, so no audit has been picked since 10-13 (32 sessions)
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
- 2026-10-15 19:05-19:07: the NVIDIA GPU fell off the bus at 18:26 (kernel NVRM fault; `nvidia-smi -L` "Unable to determine the device handle"; `vulkaninfo --summary` Intel RPL-S + llvmpipe only). SKIPC's land job set `needs: maintainer`; the picker's step 2 (`scripts/loopctl.py` ~387-393, land a parked card with a branch) does not test `needs:` (step 1 does, ~383), so the next lane re-took the same land two minutes later (reports `20261015_1906_land_SKIPC.md`, `20261015_1907_land_SKIPC.md`). Both SKIPC and VRIDE had to be set `blocked` by hand.
- Nothing in gates.py / land.py checks the adapter: SKIPC's build gate rendered on the iGPU and moved 89 goldens (33 declared + 56 the diff cannot reach), VRIDE's the same; each session spent turns proving it was the adapter (`lead_tmp/vride/null_main`). RULES has the lesson three times over (lost adapter).
- The maintainer's yes to STATUS question 49 (INBOX 2026-10-15): the loop may run a reviewed sweep of the `openbf-wt/*-scratch` worktrees (40+, `clse2b` 32 GB; lead_tmp 398 GB exclusive). `git worktree remove --force` is refused by the guard when typed (memory: guard-denies-worktree-remove-force), so the sweep is a loopctl command the maintainer approved, not a hand-typed removal.

- **Audits stopped (found by the planner, 2026-10-15 19:15):** `lanes/lane3.pid` = 6448, written 2026-10-03 06:34 by a lane-3 launcher that exited on STOP at 07:36 that day. 6448 is now a THREAD of Discord's renderer (`/proc/6448/status`: Name ThreadPoolForeg, Tgid 6395). `os.kill(6448, 0)` succeeds for a thread id, so `alive()` says lane 3 lives, and step 3 (`lane == RESEARCH_LANE or not lane_alive(RESEARCH_LANE)`) leaves the audit to it: `sessions_since_audit` = 32, last audit 2026-10-13 05:48. The planner's move of the stale file was refused by the guard; the maintainer is asked in STATUS to delete it.

## Built 2026-10-15 20:20 (branch loopt 3913acb) - re-scoped
- (1) step 2 skips `needs:`; (4) `alive()` checks Tgid, `lane_alive()` also the `autoloop.sh` cmdline; (3) `loopctl.py worktrees [--yes]`. Self-test PASS; mutations: drop the needs test -> "a parked card with a branch and needs: is not a landing" FAILS; drop the Tgid test -> "lane 3's file holds a live process's THREAD id" FAILS.
- (2) moved to card LOOPA: the oracle guard refuses a builder's edit to scripts/gates.py / land.py ("guards"); it waits on `[approvals.LOOPA] oracle = "guards"`.
- The launcher's own removal of `lanes/lane<N>.pid` on exit (a trap in scripts/autoloop.sh) is NOT done: two lanes run that script now and bash reads a script as it runs (memory: running-script-edit-breaks-the-chain). The Tgid + cmdline check makes a stale file harmless; add the trap when the loop is stopped.
- `worktrees --yes`: the dry run lists 42 candidates of 60 trees (`lead_tmp/loopt/worktrees_dry.txt`); the `--yes` run was refused by the session's auto-mode permission classifier, so it is the maintainer's to run (`python3 scripts/loopctl.py worktrees --yes` from ROOT) or to allow.
