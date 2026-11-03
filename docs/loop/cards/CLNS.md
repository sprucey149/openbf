---
id: CLNS
title: Cleanse scripts (2026-09-29): the loop/bake/gate tooling in scripts/ — stale docs, dead paths, script bugs; RBKM if it fits
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
The cleanse rotation's `scripts/artifacts tooling` area (LAST_CLEANSE): the loop's own gates and bookkeeping.

## Evidence
- A `rebake=` group that rebake.sh does not define baked nothing and passed (VERIFIED: `OUT=… BAKE=/bin/true scratch_bake.sh map` exit 0, cmp SAME=0); card AUDRB declares `rebake=<the audio groups>`.
- RBKM: `rebake=maps` skips tat1/tat2.
- loopctl `alive("0")` is True, so a pid-less claim dir was held forever.
- re_check.py crashed from a worktree (the corpora are gitignored). sim_gates.sh lost a missing-hash failure through `| tee`.

## Notes
Cards opened: SPL9 (game/src/assets.rs 1,514 lines), GRDC (claude_guard.sh -C bypass, needs the maintainer).
