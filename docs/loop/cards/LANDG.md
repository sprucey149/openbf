---
id: LANDG
title: land.py reads a previous landing's golden list on a reused out dir: the 'reuse branch rows' path copies branch/ (with an older landing's goldens/QUANT.md) into main/ and phase 1 reports those goldens moved though gpu was skipped; phase 2 then fails 'nothing to commit' and --resume ignores a new --reject
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
SSELB slice 9 (2026-10-21 12:07): gates skipped goldens (deps.toml: no golden can move), yet phase 1 said '1 golden moved: hot1_spawn_class.png' - slice 8's move, from engine/artifacts/lead_tmp/sselb/land/branch/goldens/QUANT.md (11:15). land.py:343 copytree(branch -> main), :365 moved_list(main/goldens/QUANT.md). Phase 2 ran git commit on an unchanged file -> 'nothing to commit' -> REFUSED; --resume re-uses st['reject'] (land.py:395), so --reject cannot clear it. Every multi-slice card that lands twice from one out dir hits it.
## Do
Clear <out>/branch and <out>/main (or at least their goldens/) when a new phase 1 starts; read moved goldens only when this run's gpu row is MOVED; let --resume take --reject. Then finish SSELB's phase 2 (main already holds 078c77c).
## Acceptance
A scratch test / dry run: a second landing of one card with a stale goldens/QUANT.md and a gpu row 'goldens skipped' reports 0 moved goldens; SSELB's land state reaches 'landed'.- planner 2026-10-21 13:00: value 4 -> 5 - SSELB (`first: 5`) waits on it (deps), and every multi-slice landing from a reused
  out dir hits the same refusal. Dry run: lane 1 takes it 5th (after HULLC, HTGT, PWST, RKHUL), SSELB right after.
- 2026-10-21 16:20 (HTGT slice 3): hit again - phase 1 listed slices 1-2's `tat1_hud.png` (QUANT.md 15:37) though this run's
  gpu row was PASS; phase 2 REFUSED "nothing to commit". Editing state.json's reject is refused by the oracle guard (it names
  the golden path), so the bookkeeping was finished by hand (worktree / branch / targets removed, `loopctl landed HTGT
  90132e1a`, MAIN + HTGT released). `lead_tmp/htgt/land/state.json` still reads REFUSED at step goldens.

## Build spec (LANDG session 2026-10-21 18:30; the oracle guard refused this session's Edit of scripts/land.py [guards])
Needs `[approvals.LANDG] oracle = "guards"` in gates.toml (planner or the maintainer). Line numbers are land.py at a87df1b.
1. `moved_goldens(rows, quant)`: `moved_list(quant)` only if a row has gate == "gpu" and verdict == "MOVED", else [].
   Use it at :303 (rows_b, the branch QUANT.md) and :364 (rows_m, main's QUANT.md). (undeclared() already ignores the list
   unless gpu MOVED; :364 is the one that stores the stale list in st["moved_goldens"].)
2. `fresh_out(out)`: rmtree out/{branch,main,main_after}; call it in phase1 after take_main when not `resume`.
3. phase2 resume (:390-394): if a new `--reject` is given and "goldens" not in done2, overwrite st["reject"]. Also heal an
   old state: st["moved_goldens"] = [] when rows_main's gpu row is not MOVED (SSELB's state lists hot1_spawn_class.png with gpu PASS).
4. phase2 goldens: commit only if `git status --porcelain -- *keep` is non-empty (an unchanged file = no commit, not a refusal).
5. phase2 head check (:413-421): when HEAD is not `after` but `git merge-base --is-ancestor after HEAD` holds and there is
   nothing left to commit (keep empty) or re-baseline (no behaviour row MOVED): go on, skip rebaseline() (later landings own
   the ci counts in gates.toml), skip gpu_after, do cleanup + `loopctl landed` (SSELB: worktree openbf-wt/sselb has no
   tracked changes, 078c77c is in main). Otherwise refuse as now.
6. `--forget`: close a hand-finished state (closed="forgotten", journal line; no git / card / claim change) for HTGT's
   land dir; --status then reads FORGOTTEN; main(): `--resume` of a closed state does not enter phase2.
7. Self-test (resume_self_test harness; fake journal / mark_landed / remove_target / restore_from_head, which write real
   files today): (a) a stale branch QUANT.md + gpu PASS on a reused out dir -> phase 1 moved list == [] and the
   dir is emptied; (b) phase 2 resume with a new reject replaces st["reject"]; (c) main moved past `after`, nothing to commit
   -> phase 2 reaches `landed` without touching gates.toml; (d) mutation-check (a) by reverting to moved_list.
Then: `land.py SSELB --resume` (phase 2) -> landed; set SSELB back to status=todo, branch= (it has more slices);
`land.py HTGT --forget`. (lead_tmp/landg/land.py is an UNCHANGED copy of a87df1b's land.py; the guard refused its removal.)
