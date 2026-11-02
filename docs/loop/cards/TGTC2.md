---
id: TGTC2
title: The target sweep misses capture and audit scratch: cover `<card>-render_target`, `<card>-scratch_target`, `audit-<card>-*_target` and the matching `openbf-wt/<name>` worktrees once the card is done/parked or the audit has ended — live-claim and running-process checks first
milestone: M0
value: 5
phase: build
status: done
deps: LANE3
files: scripts/loopctl.py
declares:
needs:
effort:
branch: tgtc2
resume: landed a7196742 2026-10-16
created: 2026-09-30
---

## Why
The maintainer's INBOX note 2026-09-29 22:05. PXWA and PXWB are `done`, yet `lead_tmp/pxwa-render_target` (23.3 GB real) and
`pxwb-render_target` (23.8 GB) and their detached worktrees `openbf-wt/pxwa-render` / `pxwb-render` remain. Their names
match no card id, so TGTC's sweep never sees them. The audits leave `audit-<card>-*-scratch_target` dirs and
`openbf-wt/audit-*-scratch` / `dshn-scratch` worktrees too (the orientation lists five). lead_tmp held 119 GB real on
09-29, about half of it this dead scratch. deps LANE3 only because both edit loopctl.py.

- **State at 2026-09-30 00:12 (planner):** the pxwa/pxwb render targets and worktrees are gone already (removed by hand); lead_tmp has no `*_target` but `kaad_target`. Left: the detached worktrees `openbf-wt/audit-jwal-816a9e1-scratch`, `audit-jwal-a8da834-scratch`, `audit-jwal-hyb-scratch`, `audit-jwal-scratch`, `dshn-scratch`. The rule is still needed for the next capture or audit.

## Outcome (2026-10-16, lane 1) - LANDED a719674
render targets/trees, cmdline holders, files: normalisation + check warnings, note_owner live guard; self-test 17 new cases,
6/6 mutations killed; real targets --yes freed 0 GB (nothing dead today). Report reports/*_card_TGTC2.md.
