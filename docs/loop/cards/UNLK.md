---
id: UNLK
title: BF2 class unlocks: complete score and reset/re-lock research and validate SSELB's existing 8-point officer / 12-point special gate, silhouette and text
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
UAUD2 (analysis/unit-audit-bf2.md §3.8) originally found the BF2 officer/special class gate missing.
SSELB subsequently added the gate and its presentation. This card retains the score/lifecycle research and full
validation work below; the original port-gap description is historical.

## Evidence
- Setter: B2 FUN_0026a340 stores PointsToUnlock 0x714b2463 with '%d' (ELF 0x61fc20) at class+0x374 (186907-186909).
- Disc (census2.py): 8 on every officer (clone commander, MagnaGuard, Bothan spy x3, Imperial officer), 12 on the jet troopers,
  droideka, Wookiees, dark trooper; no other fielded class. RAM agrees (verifier V11).
- PCSX2: lead_tmp/unit-audit/pcsx2_bf2/{rep,cis,end,hot,cor1g}_sheet.png - the class select at battle start shows "Get 8 /
  12 more points to unlock!" on exactly those classes and draws them as black silhouettes; AI soldiers of those classes are
  already fielded ("Unit Count: 3 Max: 4").

## Acceptance
- RE ([O]): the reader of class+0x374 - which score it compares (the player's points this battle? kills? CP captures?),
  whether a class re-locks (on death, on a new battle), whether bots are ever gated, and BF1's absence of the rule.
- Then a build card (after SPSEL gives the spawn screen a class choice): the gate, the silhouette, the text.

## Notes
- Created by UAUD2. The player's score model itself may be missing in the port; the RE should say what feeds it.

## Planner note (2026-09-30, M-ALPHA)
`defer: post-alpha` — not needed for the alpha's six showcase maps or its checklist. Value 3 (kept).

## Planner note
Planner 2026-10-21 20:50: the maintainer (INBOX 17:50 item 4) wants BF1 unlocks and a career unlock too: cards UNLK1 (BF1 tiers), UNLK2 (career counter), UNLK3 (stages + the class-select line) follow this one.

## Reconciliation (2026-10-25)
- The later `analysis/re-spawn-flow.md` §4.3 already records the reader `FUN_00364290`, its four callers and
  `FUN_00361900(stats, 1)` reading the signed non-hero points at stats+0x1fc; it also records the human-only refusal
  path, class-cap test and separate droideka pool check. Those are existing [O] UNAUDITED claims, not re-derived here.
- `analysis/re-event-messages.md` §9.1 records the scripted points table, separate hero score, ordinary kills,
  suicide/team-kill deductions and capture/assist credit. Its unread special-event and gate branches remain unread.
- Current source consumes `PointsToUnlock` in `game/src/roster.rs`, passes it to `UnitClass`, and refuses a human's
  accept through `Sim::class_refusal`. `game/src/spawn_page.rs` formats the deficit and refused mask;
  `spawn_preview.rs` applies black tint. These paths exist; do not implement the original missing-feature premise again.
- `game/src/main/windowed.rs` writes slot0's gate input from `EventFeed::stats` before `advance_with`; the feed scores
  kill/death/capture events. Existing synthetic tests manually set points: they do not prove an actual scored event
  changes the gate, or the PS2's reset/re-lock behavior. Verify that caller path, including catch-up ticks.
- Still owed: every writer/default/reset of the compared score, death/new-battle/side-change and downward-score
  crossings; event-to-score map including hero exclusions and special bonuses; bots and BF1 primary evidence;
  real roster and rendered gate/text/silhouette checks. Original Acceptance is unchanged; no new build card opened.
