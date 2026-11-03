---
id: MUSB1
title: BF1's music state rule checked live: the manager's state word against the reinforcement counts over one BF1 round on the PS2, then drop core/src/music.rs state_bf1's PROVISIONAL label or fix its thresholds (item 5 of ANNC3, split off: needs a PCSX2 session)
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
Filed by ANNC3 (2026-10-21) from its item 5 (MUSV's report): BF1's music state rule is still PROVISIONAL (core/src/music.rs state_bf1; analysis/re-ambience-music.md section 11). ANNC3's session had no RTX (adapter lost), so no PCSX2 session.
## Evidence
- engine/artifacts/lead_tmp/ambm/tools/ram_music.py reads the manager's state word.
- MUSV found a windowed battle and a headless one cannot be compared tick for tick (an idle human sits in a slot); compare by ticket counts, not times.
## Acceptance
The RAM log's state changes over one BF1 round against the port's MUSIC_LOG state changes for the same ticket counts, pasted; then the label dropped or the thresholds fixed (a unit test of the fixed thresholds).
