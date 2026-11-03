---
id: VRSV
title: Bots keep a free vehicle for each human teammate near it (the maintainer, 2026-10-07): while a human is within a range of an unoccupied vehicle the bots leave one claimable vehicle per human alone, released when the human walks off or has stayed in range about 20 s without boarding
status: archived
---

Historical technical record. Recorded state before archival was `todo`.

## Why
The maintainer, 2026-10-07: "Maybe if there is an unoccupied vehicle(s) near the user, the bots will save 1 vehicle per teammate user in case the user wants to use it. There should be a range and a timer for this though, like if the user gets so far away, the last vehicle is up for grabs, or if the user is within the vehicle range for say like 20 seconds and hasn't jumped in, it is also up for grabs again."

## Evidence
- Today (`ai/ride.rs` `claim`): a bot on foot whose post is farther than 80 m (`BOARD_TRIP`) claims the nearest free vehicle within 75 m (`BOARD_SEEK`) that no other bot has claimed and whose graph reaches its post; a claim not boarded in 30 s is dropped and the bot waits 30 s (`BOARD_GIVEUP`, `BOARD_BLOCK`). A command walker (AT-AT, AT-TE) is always manned. Nothing in it knows a human is near: the bots will take the vehicle you were walking to.
- The numbers were measured against the PS2's bots (WAI2: 8-71 m for a walker, once 146 m); the PS2 has no reservation.

## Acceptance
- A vehicle within a RANGE of a human teammate (provisional: 40 m, the loop measures what fits the maps) that no bot has claimed is held back from bot claims: one vehicle per human on the team, the nearest first. It is released when the human is farther than the range, or has been inside it for 20 s (provisional, the maintainer's number) without boarding; after a release it is not held for that human again until he leaves the range and returns. A command walker is never held back. Seats of a vehicle the human already sits in are not affected (a bot gunner may still join).
- Tests for the hold, the distance release and the timer release; the sweep and arena baselines are identical with no human on a team (the bots-only runs).
- A named deviation in FEEL_NOTES.

## Notes
Filed 2026-10-07 by Claude in chat from the maintainer's questions. His words are quoted; the code facts were read by Claude from `core/src/sim/seats.rs`, `sim/guns.rs` and `ai/ride.rs` the same day. Deps: VSWP (the seat rules a human meets first) and TURR (which owns when bots man seats).
