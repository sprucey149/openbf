---
id: SSEL
title: The start-of-level and respawn flow: pick a SIDE (or random), then a command post, then a class at the start of a level; post and class after a death - the PS2 BF2 screens, on the sim's real spawn choice
milestone: M5
value: 5
phase: re
status: done
deps: SPSEL
files:
declares:
needs:
effort:
branch:
resume: RE done 5aa7304 2026-10-20 (analysis/re-spawn-flow.md [O]); split into build cards SSELA (sim: the stages, side choice, accept, no wave for the human), SSELB (the screens + camera-shot backdrops), SSELC (the death camera); report reports/20261020_0412_card_SSEL.md; audit INCOMPLETE / UNAUDITED 2026-10-25 06:15 UTC — restore ALL held evidence or reviewed scope reset; report engine/artifacts/loop/reports/20261025_0615_audit_re-spawn-flow.md; no unchanged-input retry
created: 2026-10-19
first: 5
---

## Why
(the maintainer, 2026-10-19, and Reddit's top ask after the release) At the start of a level the player should choose a side (or random), then a command post, then a class; after a death, the post and the class again. Today there is no side pick at all, the post cursor on the spawn screen is UI-only (the sim picks the spawn node itself), and the class row is real (`Sim::spawn_choice`).

## Evidence
`ui/src/spawn.rs` (the spawn screen: class row real, post cursor UI-side); card SPSEL (the spawn POST in the sim, the PS2's spawn-map pick); card BMAP2 (the spawn screen's map view); card CFOVM (the PS2 camera FOV in the spawn-select and death cameras); card UISN (the screens' sounds); `analysis/re-ambience-music.md` §6 (the PS2 spawn-screen state machine, already read). The in-game unit select (the rotating trooper line-up) is likely close to the same on PC and PS2 - verify against a PCSX2 capture before reusing a layout. PC-derived layouts are NOT the PS2's: the front-end shell differs between the platforms.

## Notes
- planner 2026-10-19 16:25: the death camera belongs here too (what the PS2 shows between a death and the spawn screen: does it follow the body, turn to the killer, how long) - measure it in the same PCSX2 captures; card CFOVM is its FOV. The maintainer (INBOX 10-19 (a)): the in-game unit select (the rotating trooper line-up, framed by the map's camera shots) is probably the same on PS2 and PC, so its layout may follow the PC game's, checked against a PCSX2 capture; the shell is not (BSHL).
