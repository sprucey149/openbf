---
id: SQHUD
title: The squad order's D-pad cross in the HUD: BF1's SquadCommandGroup (squad_cmd_cross 64x64 at the bottom right, the four order names, the chosen arm bright with its highlight, a 2 s fade) shown at the press, in place of the cyan text line
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
SQACK (landed 83ea41b, 2026-10-03) built the PS2's orders, lines and answers; the PS2's own feedback for the press is a
small D-pad cross in the HUD, not a text line. The maintainer (INBOX 2026-09-30): squad orders the BF1 way. Today the port says
the order as one cyan line of the message box (`game::event_feed::order_line`), which the PS2 does not do for BF1.

## Evidence
`analysis/re-squad-orders.md` section 2 [V][L]: element `SquadCommandGroup`, texture `squad_cmd_cross` 64x64 at
(-120, -50) from the screen's bottom right, four `gamefont_tiny` labels at +-25 (`game.squadcommand.field.up|down|right|left`
= MOVE OUT / HOLD POSITION / FOLLOW ME / AT EASE), a `squad_cmd_highlight` quad per arm; the chosen arm at alpha 255
with its highlight, the others at 127; the element's alpha = timer x 255, timer 1.0 falling 0.5 a second (2.0 s; live
1.97 s in 11 scenes). PS2 frames: `engine/artifacts/lead_tmp/sqack/scenes/hud_sheet.png`. Both textures are already
loaded for the dev marker (`game/src/main/battle.rs::squad_icons`). `Event::Command { slot, kind, .. }` is the press
(kind 0 follow, 1 hold, 2 at ease, 3 move out).

## Acceptance — a check to RUN and paste
1. A HUD layout test (`game/tests/hud_layout.rs` style): the cross's rect, the four label anchors and the chosen arm's
   alpha at t = 0, 1.0 and 2.0 s against the numbers above (anchored bottom right at 16:9 and 4:3, Hor+).
2. A PCSX2 pair: the port's HUD crop beside `hud_sheet.png`'s frame for FOLLOW ME, label text and position within 2 px
   at the PS2's resolution.
3. The cyan order line is gone on a press (the cross replaces it). **The count of units doing it stays - the maintainer's own
   ask, see the planner section below** - drawn with the chosen arm, and a test pins it (FOLLOW ME with five in reach
   shows 2; HOLD with three shows 3; nobody in reach shows no number).
4. A golden of the HUD with the cross up (declared).

## Planner 2026-10-03 03:35 - value 4 -> 5; the unit count comes back
The maintainer, INBOX 2026-09-30 12:05 (his correction to the squad note): "keep the brief on-screen command text ('Follow me'
and the like) and show how many units are doing it. The teal markers go." SQMK built the count; SQACK took it out
again because the PS2 shows none (its acceptance 6, and this card's old line 3: "no unit count anywhere"). What the maintainer
said outranks the PS2 here (the prompt's "faithful yet modern": follow what the maintainer has said). So:
- The cross is the PS2's (Evidence above). The count is OURS, a named deviation: a small number beside the chosen
  arm's label (same font and alpha as the label), the units that TOOK the order. Takes arrive at each bot's own
  think, so count `Event::OrderTaken` for that press as they come (all are in within about 0.5 s); for FOLLOW ME it
  is the followers (at most 2), not everyone who answered. A release order (MOVE OUT / AT EASE) shows the number
  released. One line in `FEEL_NOTES.md`.
- The PCSX2 pair of acceptance 2 is taken with nobody in reach (no number), so the pair still compares like with like.
- Raised to 5 because main today shows no count at all - a regression against the maintainer's note until this lands. It shares
  `ui/src/hud.rs` and `game/src/main/battle.rs` with BMAP, so the two run one after the other.
Undo: `loopctl.py set SQHUD value=4`, and restore line 3 from `lead_tmp/planner_0330/SQHUD.md`.

## Landed 2026-10-03 06:14 - 2a3aef4 + golden 56fd080 (report `reports/20261003_0615_card_SQHUD.md`)
The cross is `ui/src/hud_squad.rs`, its state `EventFeed::squad`, BF1's font by the new `bake fonts`. Acceptance run
on main: every measured edge of cross / highlight / label in PS2 frames F3, H3, A3 within 1.9 PS2 px (centre and
label rows fitted to those frames - not independent); counts 2 / 3 / none; golden tat1_hud.png. Open: the element's
own placement rule is 2.6 / 2.3 px off the measurement (projection not traced), label rows PROVISIONAL.
