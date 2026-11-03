---
id: SPL10
title: walls.rs and manifest/src/lib.rs are both under the 1,500 cap now (10 and 1,291 lines) — only the original gates remain
status: archived
---

Historical technical record. Recorded state before archival was `todo`.

## Why
Cleanse CLNC (2026-09-30): `scripts/size_report.sh --strict` exited 1 on walls.rs (2,086; it grew through JWAL / WTUN / LEAK3 / WPSH / STHOP) and neither file was named by an SPL card. CLNC waived both in `scripts/size_waivers.txt` pointing here.

## Acceptance
`scripts/size_report.sh` lists neither file over 1,500 lines, or the waiver line gives the 'no seam' reason; every gate PASS with nothing declared (walls.rs carries the FULL sim set incl. the leak census; manifest the scratch bake at DIFF=0); the two waiver lines deleted when the split lands; CRATES.md rows current. One file per session (SPLIT_RECIPE).
- 2026-10-06 planner: `engine/CRATES.md` taken out of `files:`. The picker refuses a card whose files overlap a card in progress, and 22 open cards listed the map, so any one of them held the other 21 (a chain card in progress held WADE and WTHR). The map's row for a new or moved source file is still owed in the same branch (`scripts/size_report.sh --check-map`); a rebase conflict in it is one line. Undo: add the path back.

## Independent remaining split (2026-10-26)
SPLENV owns the remaining source extraction as a separate card. This preserves
all original acceptance, scratch-bake equality and prior validation debt.

## Cleanse note (2026-10-29)
`wc -l`: core/src/collision/walls.rs 10 (queries.rs 1,211, tests.rs 1,164), manifest/src/lib.rs 1,291. Both files are under the 1,500 cap; nothing named here is over it. Status is left for the lead.
