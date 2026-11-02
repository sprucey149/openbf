---
id: FIDA
title: The fidelity gate looks at a second seed block for an ALL-MAP sum too: recycled (all maps) over 8 seeds has a tolerance of 12 against a block-to-block spread of 10.7 (1 sd), so about 1 landing in 8 that re-rolls the seeds is refused on noise, every time it is tried - OBSV was (58 against 40; 24 seeds 145 -> 150, t +0.26)
milestone: M0
value: 5
phase: build
status: done
deps:
files: scripts/gates.py
declares: oracle=guards
needs:
effort:
branch: fida
resume: landed 33d9c78e 2026-10-16
created: 2026-10-12
alpha: yes
---
## Why
`scripts/gates.py`'s fidelity group re-runs a PER-MAP failure on seeds 9-16 against main's own 9-16 values and keeps it
only if both blocks are worse (card FIDC, 2026-10-11). An ALL-MAP value has no second look (`fidelity_confirm`: "agg
stays"). For the two metrics that are SUMS over the seeds (`FID_SUM`: recycled, noprog) one 8-seed block is noisy.

## Evidence
- Main's own all-map recycles, seeds 1-8 / 9-16, three mains of 2026-10-12: 49 / 61 (19708fa), 30 / 44 (d096ae7),
  40 / 48 (5fbbbd4) - `lead_tmp/fidelity_rebase_20261012{b,c,d}/gates/fidelity/`.
- Per seed on main 17ae28e (24 seeds, `lead_tmp/obsv/fidelity_noise.txt`): mean 6.04, sd 2.68. So an 8-seed sum has
  sd 7.6 and the difference of two independent ones 10.7 [D]. `tol_agg_abs = 12` is 1.12 sd: a change that re-rolls
  the seeds and moves nothing is refused 13 % of the time [D, normal approximation]; with both blocks required, 1.7 %.
- The case: card OBSV, branch 58 against 40 on seeds 1-8, 49 against 48 on 9-16, 43 against 57 on 17-24; 24 seeds
  145 -> 150, paired t +0.26. The same binary is refused every time it is tried: the refusal is deterministic.
- The other way matters as much: the gate must still catch a real rise. RLOOP's (a real cost): 46 against 30 and 58
  against 44 - worse on both blocks by more than 12, so it is still refused under the proposed rule.

## Notes
- 2026-10-12 17:05 planner: filed with `needs: maintainer` - the gate script is a protected path (the oracle guard refuses a
  builder; GRFG's edit took the maintainer's approval) and loosening a referee is his call. STATUS question 37. On a yes: the
  planner adds `[approvals.FIDA] oracle = "guards"` and clears `needs:`.
- Not proposed: a wider tolerance (it would let a real +15 through on one block).

## Planner 2026-10-12 19:30 - a second change to ask for with this one (STATUS question 37)
`land.py` re-seeds `[fidelity]` only for a declared or approved move. A landing that makes a value BETTER leaves the
old, lower baseline, and the next card may give the gain back unseen: CLSE2B passed the gate at 19:05 with an enemy
within 20 m at 11.46 % while main stood at 12.73 (the baseline still said 11.14, from before DEFS and CLSE4). Wanted
with this card, same protected script: at `--commit`, when the landing's own main rows are better past tolerance,
write them as the baseline (seeds 1-8 from the landing's run; the confirm block from a `--fidelity-both` run of
main, 5 minutes) and name the landing in `source`. Until then the planner re-takes the baseline whenever a report
lists "better" rows (RULES, Landing).
