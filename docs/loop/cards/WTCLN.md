---
id: WTCLN
title: Scratch worktrees remove themselves: a scratch tree stays clean (outputs to lead_tmp/<card>/, the asset symlinks excluded so no tree reads dirty), the session or the landing that made it removes it with a plain git worktree remove, and the planner's pass lists any scratch tree older than one session as a problem
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
INBOX 2026-10-21 20:30 (Claude, the maintainer's yes; `first:` "low but before the next cleanse"). 35 trees under `openbf-wt/`
(planner 10-22 00:00, `loopctl.py worktrees`), 21 of them `*-scratch`; VS Code lists each as a dirty repository. A hook
blocks the forced remove, so nothing removes them and `worktrees --yes` waits on the maintainer. Planner's reads (10-22 00:00):
- **Every tree reads "dirty 5" because of its 5 asset symlinks** (`?? PC_swbf2_exe community_mods engine/artifacts
  engine/assets_baked extracted` in `openbf-wt/hvya` and `loopa-acc-scratch`): `.gitignore`'s `dir/` patterns do not
  match a symlink. Untracked-but-not-ignored files also make a plain `git worktree remove` refuse.
- **A detached scratch tree with a local commit is kept forever** ("commits not on main"): `loopa-acc-scratch` b6e6a1d
  "scratch: touch a golden dep" (card LOOPA done). About 12 such trees belong to done cards.

## Do (all in `scripts/loopctl.py` - no gate code: land.py already calls `loopctl.py landed`, the launcher `postmortem`)
1. `start` and the scratch recipes: the 5 links listed in the common git dir's `info/exclude` (no-slash patterns), so a
   tree is clean; a fresh tree's `git status --porcelain` is empty.
2. `postmortem` (after every session): a clean `*-scratch` tree that no open card names and whose card is done, or that
   this session made, is removed with a plain `git worktree remove` (no force flag, ever); its diff, untracked files and
   own commits (format-patch) saved first to `lead_tmp/worktree_sweep/<date>/`, as `worktrees --yes` does. A dirty one is
   named in the journal line and kept.
3. `landed <ID> <sha>` does the same for the card's own scratch trees (`<id>-*-scratch`, `audit-<id>-*`).
4. `scoreboard` prints a `worktrees:` line; a scratch tree older than one session is a PROBLEM the planner lists in STATUS.
5. RULES.md line for the planner to merge: a session's scratch outputs go to `lead_tmp/<card>/`, its builds to the
   card's target dir, never inside the tree.
Never touch a branch tree of an open card or a tree an open card's text names (the `worktree_mentions` rule stands).

## Acceptance
- `loopctl.py self-test` gains fixtures: a clean scratch tree of a done card is removed; a dirty one is kept and
  reported; a parked card's branch tree is kept; a fresh `start` tree has an empty `git status --porcelain` (paste).
- On the real repo after one `postmortem`: `git worktree list` before / after counts pasted; each removed tree has its
  saved diff under `lead_tmp/worktree_sweep/`.
- After a day of the loop: `git worktree list` shows only branch trees of open cards (+ trees open cards name).
The maintainer still runs `python3 scripts/loopctl.py worktrees --yes` once for today's trees (STATUS).

## Notes
- planner 2026-10-22 00:00: filed from the INBOX note; needs no GPU (scripts only), so it can land during the RTX outage.
