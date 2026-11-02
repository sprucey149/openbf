---
id: FXC
title: Chunks mungefs does not parse that the PS2 may draw: shdw (shadows), CLTH (cloth), bnd_, font — a census first (fx__ went to FXP, done)
milestone: M1
value: 4
phase: re
status: done
deps:
files:
declares:
needs:
effort: high
branch:
resume: closed: census in analysis/re-chunk-census.md (81dd465); cards VSHD, CLOTH; note on BNDY
created: 2026-09-25
alpha: yes
---

## Evidence
analysis/PLAN_LOG_2026.md §ARCHIVED 2026-09-25 HANDOFF-OPUS.md HOLD 4. Only `Effect("Water")` is released (`analysis/re-water.md` §5). Vehicle `zaa_` banks belong to VEH0; `coll`
response to COLR; `plan`/`path` AI use to AIDS.

## Note (cleanse CLNM2, 2026-10-05)
The title still lists `font` as unparsed: `mungefs::font` has read BF2's bitmap fonts since card BHUD (b06686f). The census this card asks for should start from shdw, CLTH and bnd_ only. `mungefs::fx` and `mungefs::particle` no longer cite this card for the unparsed `fx__` effects (Precipitation / Wind are WTHR's, Lightning's flashes and bolt WTHL's; ColorControl and the rest have no card).

## Done (2026-10-16, lane 2)
Census + RE in `analysis/re-chunk-census.md` (81dd465). shdw -> new card VSHD (M1, 4): vehicle shadow volumes, proved by
a PCSX2 knock-out on BF1 NAB1 (9.5 % of the frame, on/off 0.63). CLTH -> new card CLOTH (M1, 5): BF2 cloth, the Hoth
snowtroopers' skirts first. bnd_ -> note on BNDY (empty boundary scope on BF2 UTA1 / MYG1 and 6 more; no chunk on BF1
YAV1 / YAV2). Report `reports/20261016_0715_card_FXC.md`.
