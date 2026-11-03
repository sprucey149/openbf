---
id: PKHD
title: loopctl's busy-files test: a parked card with needs: set holds no files - the planner sets such cards blocked by hand today (SNPR, HJAM) so the cards behind their files can run
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
Planner 2026-10-16 04:35 (`reports/20261016_0435_planner.md` item 4): a parked card waiting on the maintainer (`needs:` set) still
holds its `files:` in the picker's busy set (`order()` / `eligible`: `status in (in-progress, parked)`), so SNPR held
`ai/behave.rs` and kept every value-5 bot card (TURR, TPOS, TEAM, TCMP, HVYA) out of the picker; HJAM held `route.rs`.
The workaround (set them `blocked`) loses the land-job route when the maintainer answers.

## Notes
After it lands the planner may set SNPR / HJAM back to `parked` with `needs:` (their resume lines name the maintainer's questions).

## GitHub checkpoint recovery (2026-10-24)
