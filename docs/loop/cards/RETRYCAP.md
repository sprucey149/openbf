---
id: RETRYCAP
title: Cap attempts per card and route a stuck one elsewhere: a `tries` counter on the card (a session that ends without landing or closing it counts one), after 3 builds / 2 RE sessions the picker stops handing the card out, parks it with a `resume:` naming what each try found, and files a one-line Question for the maintainer or a planner re-cut
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
Current source has a consecutive unchanged-card guard: `MAX_NO_PROGRESS = 3`
and `pick()`'s `attempts` entry count matching card fingerprints, resetting the
count when the card changes. That does not bound cumulative unfinished sessions
across resume edits or implement the requested two-session RE cap. Build only
that missing scope. The session time limit remains in LOOP_PROMPT's budget rule.
Planner 2026-10-24 corrected the old no-attempt-guard premise; acceptance below
still applies. AUDPRE separately owns missing-input holds for incomplete audits,
which have no assigned card and must remain unaudited.

## Evidence
- `scripts/loopctl.py`: `pick()` takes the top eligible card; `postmortem()` (autoloop.sh after each session) detects "a claim left or a card in-progress with no report" and writes the DIED line; `resume:` and `status=parked` already exist, and a parked card with no `branch:` becomes an ordinary card again once its deps are done (card LRES), so `parked` alone does NOT stop re-picking - the rule needs its own field the picker honours.
- Audit counts (research README): 12 of 92 sessions were resumes after a death; recurring failure class = false absence claims.

## Notes
Smallest version first: if the planner routing (item 2) is too much, ship the counter, the cap in `pick()` and the orient line, and file the routing as a follow-up. Touches `scripts/loopctl.py` (the picker); the self-test is the safety net, so run it before and after. The loop is stopped (STOP file) at filing time.

## GitHub checkpoint recovery (2026-10-24)

## Planner follow-up (2026-10-24 17:51 UTC)

Keep the current duration-independent failed-attempt rule, including short launcher
deaths and failed stand-downs. Successful in-session slice landings and finished
RE phases stay exempt as locally validated. A short duration alone does not prove
a failed attempt should be ignored; preserve the cap against repeated failed starts.
This is a reversible planner default, not a new policy/code change. Supervisor
loads loopctl once; restart it through its normal controls to consume newer code.
