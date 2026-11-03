---
id: LDET2
title: land.py detach follow-ups from LDET's own landing: a resume/commit by the lane pid that already holds MAIN is refused; a detached run's refusal never reaches --status; phase 2 keeps the legacy flag
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
Found landing LDET with the new land.py (report reports/20260930_0105_card_LDET.md): `land.py LDET --commit --detach` exited at once with 'MAIN is held by a live session' — MAIN was held by the same lane pid (6991) that asked, and the pre-LDET state.json had no main_holder; the refusal went only to land.log, so `--status` kept saying 'phase 1 finished'. A phase 2 that dies on a legacy state would also read as 'finished by the pre-LDET land.py', not DEAD.
