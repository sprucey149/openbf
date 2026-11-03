---
id: XDET
title: One cross-OS determinism run (the parry flag + `fmath`): the same replay on two OS/CPU builds hashes the same
status: archived
---

Historical technical record. Recorded state before archival was `todo`.

## Evidence
analysis/PLAN_LOG_2026.md §ARCHIVED 2026-09-25 HANDOFF-OPUS.md §Standing backlog 9; DET1 closed the known host-libm leak.

## Acceptance
The hash pair recorded, or a card per divergence.

## Planner note (2026-09-30, M-ALPHA)
`defer: post-alpha` — not needed for the alpha's six showcase maps or its checklist. Value 1 (kept).

## Planner note (2026-09-30 13:30) — multiplayer hold
The maintainer (INBOX 2026-09-30 10:05): multiplayer (LAN and splitscreen) is the LAST milestone, now **M7**, after PACK, and held
until the maintainer says it is ready. This card is the LAN lockstep's cross-machine check, so it is held with it (`needs:`). The
same-machine determinism spine (replay hashes, `fmath`, the sweep byte-compare, HASH, CLN6) stays in rotation.
