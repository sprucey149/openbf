---
id: TGTC
title: Scratch cargo targets are deleted when their card is done: land.py removes <card>_target (and the card's worktree leftovers) after a landing, parking a card removes its target, stale *-scratch_target / *_pxc_target go at the next session start, and the scoreboard prints lead_tmp's apparent + btrfs-exclusive size
milestone: M0
value: 5
phase: build
status: done
deps:
files: scripts/land.py,scripts/loopctl.py,scripts/cleanse_delete.sh
declares:
needs:
branch: tgtc
resume: landed 06651bd6 2026-09-29
created: 2026-09-29
---

## Why
The maintainer's INBOX 2026-09-28: 15 `lead_tmp/<card>_target` dirs (btrfs reflinks of `engine/target`, ~45 GB apparent /
~20 GB exclusive each) piled up in one day — ~300 GB real, nearly all of /home. He wants it automatic, in the tools
(land.py), not only in cleanse. On 2026-09-29 05:30 two more were waiting (`vturb_target`, `uaud-scratch_target`,
89 GB apparent) and the planner's `scripts/cleanse_delete.sh --yes` was denied by the classifier ("Shared Scratch
Sweep") — so the tooling must do it as part of a reviewed job step, not a session's ad-hoc sweep.

## Notes
- The maintainer also asked for the rule in the prompt. Planners may not edit the prompt; the tooling above enforces it anyway,
  and the exact prompt line is proposed in STATUS (2026-09-29) for the maintainer to paste.
- Past sessions learned to unlink a worktree's symlinks before `git worktree remove` — do it in code.
