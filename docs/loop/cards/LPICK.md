---
id: LPICK
title: loopctl next starves value-5 capture cards: the lane-preference window takes every card within ONE value point of the best, so value-4 re/build beat value-5 PXWA/PXWB forever — reorder by lane preference among EQUAL value only
milestone: M0
value: 5
phase: build
status: done
deps:
files: scripts/loopctl.py
declares:
needs:
branch: lpick
resume: landed ed4a2f4 2026-09-29; self-test = loopctl.py self-test
created: 2026-09-29
---

## Why
Planner 2026-09-29. `scripts/loopctl.py pick()` step 4: `top = [c for c in r if val(c) >= best_val - 1]`, then sorts `top`
by `LANE_PREF` (lane 1: build, cleanup, measure, re, capture; lane 2: re, capture, measure, build, cleanup). With any
value-5 card open, every value-4 card joins the window, and capture ranks last (lane 1) or after re (lane 2). PXWA and
PXWB (M1 5, capture, open since 2026-09-28) have been passed over by every session since; once UAUD2 / UTAS are done,
lane 2 would take the value-4 re cards (DEKA, FAMB, FARC, FOGC, FXP, LEAF, …) before them. The planner's value is
meant to be the ranking ("Rank = value").
