---
id: AWDM
title: Award messages and points pop-ups: what the PS2 shows when you earn an award or points, and where
status: archived
issue: 50
---

Historical technical record. Recorded state before archival was `blocked`. Current scope and ownership are on [issue #50](https://github.com/sprucey149/openbf/issues/50).

## Why
The maintainer's INBOX 2026-09-30 11:40: filed now so it is not forgotten. What the PS2 shows when you earn an award or points, and
where. Belongs with the awards system (M4). EVMSG leaves the slot for it.

## Acceptance
- RE: the award and points messages' text, icon, position, duration per game ([V]/[O]).
- Built: a PS2 versus port pair of an award and a points pop-up, text identical, position ±4 px (640×480-equivalent).

## Existing evidence (historical; not re-derived by this attempt)
- EVMSG RE 2026-09-30 (`analysis/re-event-messages.md` §2): the award lines are lines of the BF2 systemMessages element — B2 `FUN_00360320` (D:357432) builds the Locl key `game.awards.medal.<name>` (cyan (32,223,223), colour copy at 0x69a628) or `game.awards.bonus.<name>` (yellow (223,223,32), 0x69a630) from the name table at 0x62c568 and sends it to the earning player; callers D:357223 (a medal earned), D:357423 / D:357540 (a bonus switched on). The points pop-up is the `player1stats` group (§5: points + kills with a "+n" delta, 5 s hold + 1 s fade) — EVMSG builds it.

- EVMSG 2026-10-02 (landed 9d4dade): award lines are lines of the message box — `game::event_feed::EventFeed::say(text, ui::hud::message_color::CYAN | YELLOW, tick)`. BF2's points table and award sites are in `analysis/re-event-messages.md` §9.1 (RAM: `lead_tmp/evmsg/points_ram.py`); built: AI kill +1, human +2, suicide / no-killer death -1, team kill -2, capture +5 / +2 (`event_feed::points`). NOT built and not read: the 2-point kills (kinds 0x17, 0x18, 0x1b, 0x1d, 0x1e), kind 0x1f (-12), the hero's 5. The pop-up shows points + kills only (the layout has no other element).
