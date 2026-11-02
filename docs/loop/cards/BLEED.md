---
id: BLEED
title: Is `tick_bleed`'s AI-shed loop reachable from bleed at all? If not, every tick pays for a safety net
milestone: M0
value: 1
phase: re
status: done
deps:
files:
declares:
needs:
effort:
branch:
resume: RE complete: keep unconditional repair. Legal mission-drain step test and 32-case CP clamp matrix pass; see analysis/re-bleed-reachability.md.
created: 2026-09-25
tries: 1
---

## Evidence
analysis/PLAN_LOG_2026.md §ARCHIVED 2026-09-25 HANDOFF-OPUS.md §Standing backlog 8 (17B's observation).

## Acceptance
Reachable (keep, with the test that reaches it) or not (delete behind a cleanup, every gate PASS).

## Finding (2026-10-24)
Keep the loop: fixed-step CP bleed preserves the live ceiling, but an extra
mission death drain reaches the unconditional repair with bleed zero. The legal
three-ticket player-plus-two-AI death-region regression needs one despawn after
the extra drain and none in its control; both battle and actual unit census agree.
The 32-case CP matrix covers both teams, all tiers and human/handicap settings.
See [re-bleed-reachability.md](../../../analysis/re-bleed-reachability.md).
Runtime code is unchanged; comments now describe both responsibilities. The
keep-with-test acceptance is complete; the deletion/cleanup alternative was not taken.
