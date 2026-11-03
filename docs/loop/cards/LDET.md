---
id: LDET
title: No session dies waiting on a gate: land.py and gates.py run fully detached with a state file per landing (phase, step, outputs, pids), `--status` / `--resume` continue at the right step without re-running finished ones; sessions poll with a time budget
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
The maintainer's INBOX note 2026-09-29 22:40 (approved loop improvements), item 5. 12 of 92 sessions since 09-26 were resumes after a session died (13%), each costing up to an hour of wall
time. The main known cause: a session waits on background gates and dies with them (TALPH iterations 7–9, WALK
iteration 18; memory notes "long gates run detached" and "interrupted land phase 1"). land.py already keeps
`lead_tmp/<branch>/land/state.json`, but only at the end of phase 1. A death mid-phase leaves "merged + rebaked, no
state", which a fresh session must repair by hand.
