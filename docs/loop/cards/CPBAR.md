---
id: CPBAR
title: A command-post capture icon on the HUD (the maintainer's INBOX 2026-10-12 19:10): the PS2 HAS one in both games - the post symbol hud_flag_timer at the right edge at mid height, cut by a line into two colours (a neutral post: the capturing team's colour rising under grey; an enemy post: grey coming down over the owner's colour; two sides inside a neutral post: the top blinks every half second with the other side's colour while the meter is frozen; his own post: nothing) - RE done 2026-10-13, analysis/re-capture-display.md. Build the PS2's icon by the PS2's rule, plus two additions of ours for the asks the PS2 does not answer: his OWN post while it is being lost, and which way the line moves; the same numbers for every post in the black box digest
milestone: M3
value: 5
phase: build
status: done
deps:
files: engine/crates/ui/src/hud_capture.rs,engine/crates/ui/src/lib.rs,engine/crates/ui/src/hud.rs,engine/crates/ui/src/hud_tree.rs,engine/crates/ui/src/hud_draw.rs,engine/crates/game/src/capture_feed.rs,engine/crates/game/src/lib.rs,engine/crates/game/src/main/battle.rs,engine/crates/game/src/main/windowed.rs,engine/crates/core/src/sim/round.rs,engine/crates/game/src/blackbox.rs,engine/crates/game/examples/blackbox_read.rs,engine/crates/game/tests/capture_display_ps2.rs,engine/crates/game/tests/data/capture_display_ps2.csv,engine/crates/game/tests/golden_hud.rs,engine/crates/game/tests/golden_spawn.rs,engine/CRATES.md
declares:
needs:
effort:
branch: cpbar
resume: LANDED ab0d172 (report reports/20261015_1421_card_CPBAR.md)
created: 2026-10-12
alpha: yes
---

## What the RE found (2026-10-13; `analysis/re-capture-display.md`, docs cdd8981, report `reports/20261013_0642_card_CPBAR.md`)
- **The card's premise was wrong: the PS2 shows a capture icon in the 1-player HUD, in both games.** It is a code
  object, not layout data (`re-hud.md` 3 is corrected): the post symbol `hud_flag_timer` (64 x 64, baked for both
  games) at the right edge at mid height, cut by a horizontal line into two colours.
  - neutral post: the capture meter's team colour rises from the bottom under grey (line at 1 - capture / T);
  - enemy post: grey comes down over the owner's colour (line at neutralize / N); all grey = neutral;
  - a neutral post with two sides inside (meter frozen): the top alternates every 0.5 s with the other side's colour;
  - an enemy post with defenders inside: no blink (defenders do not slow the meter);
  - **his own post: nothing is drawn**, even while it is neutralized under his feet; outside a region: nothing.
- **BF2's 10-segment ring (`playerNcpstatus`) is dead data on the PS2**: 3 / 4-viewport layout only, and no code
  sends its events. It is not what this card builds.
- Seen live in both games (BF1 Yavin 4 Arena, BF2 Mygeeto): rows of the post's meters beside the HUD object's
  fields, video, save states. Strips: `engine/artifacts/lead_tmp/cpbar/sheets/*.png` (7 sheets).
- The port plays none of the post's sounds (no code names ChargeSound / DisputeSound): card CPSN, as it says.

## Design (ten lines)
1. `game/src/capture_feed.rs`: one pure function (viewer, post, units inside per team, battle clock) -> the icon or
   none = re-capture-display 3.1 steps 3-6: the FIRST post whose capture region holds the viewer's unit while he
   counts; neutral: split 1 - capture / T, bottom = the meter's team colour (grey when it has none), top grey or,
   with two sides inside, alternating each 0.5 s of battle clock with the colour of the first team inside that is
   not the meter's; enemy post: split neutral / N, grey over the owner's colour; own post: none.
2. Colours: `ui::hud::HudMessage::{OWN, ENEMY, NEUTRAL}` by relation to the viewer, on every map (the PS2's team
   table row; BF1's own HUD drew its friend green - the port's HUD is BF2's everywhere).
3. Units inside per team: a read-only `Sim` helper in `core/src/sim/round.rs` that `update_cps` calls too (one rule,
   no new state: the sim hash must not move).
4. `ui/src/hud_capture.rs`: the model + the draw, the way `ui/src/hud_squad.rs` is: the sheet as two quads cut at
   row split x 64 (UV rows 0..f and f..1); frame box x 420.99..486.40, y 194..258 of 512 x 448; Hor+ at the right
   edge (`hud_squad::screen`); `HudModel.capture`, `Hud.capture`, `HudFrame.capture` (the boxes, for tests).
5. OURS 1 (the maintainer's LOSING ask; the PS2 draws nothing): on his own post the attacker's mirror - his colour below, grey
   coming down at neutral / N - while the meter is above 0 or an enemy is inside. A const switch.
6. OURS 2 (the maintainer's "which way"; the PS2 has no sign): a small chevron at the line, up or down, from the sign of the
   meter's change this tick; none while it holds or is contested. A const switch.
7. FEEL_NOTES: a line for 5, for 6 and for the colours on BF1 maps.
8. Hidden when he is dead, with the pause menu up and after the outcome (the PS2's gates).
9. Black box: `PostRow` gains the units inside per team (schema 5); the digest prints per post the running meter as
   a fraction, its team, rising / falling / holding and "contested".
10. Not here: the post's sounds (CPSN), the ring (dead data), the 0.75 scale with two viewports (SSCR).

## Notes
- 2026-10-13 06:42 (lane 2, RE): done in one session - `analysis/re-capture-display.md` (docs cdd8981), verifier
  workflow 8 agents / 44 claims / 51 SUPPORTED, 10 PARTLY, 0 NOT. The premise (no 1-player meter; build the ring) was
  wrong on both counts; the card is re-written above. Kit: `engine/artifacts/lead_tmp/cpbar/` (ps2/cp_probe.py,
  tools/, sheets/, fixture/). STATUS question 44 corrected in place.
- The same session's notes elsewhere: BHUD3 (nine more code-drawn HUD pieces are unread), SSCR (the icon's 0.75
  scale with two viewports), CPSN (the units-inside helper serves the dispute sound).
- 2026-10-12 23:40 (planner): filed from the maintainer's INBOX note at his value (5, alpha, M3, re then build). The existing RE
  (re-hud line 199) already answers half of step 1 on paper; it is `[O]`, so the PCSX2 strip is still the first step.
- Overlaps BHUD3 (4) in `ui/src/hud*.rs` at build time: the builder fills `files:`; BHUD3 then waits on this card.
- CPSN owns the post's sounds; this card only reports whether the charge / dispute sounds play in the port.
- Undo: `loopctl.py set CPBAR value=4`.
- 2026-10-15 14:21 (lane 1, build): LANDED ab0d172, no goldens, every gate PASS. Capture icon by the PS2's rule (4,332 of 4,338 PS2 rows, = replay.py), ours: own post losing + direction pointer (FEEL_NOTES); black box schema 5 post rows. Report `reports/20261015_1421_card_CPBAR.md`.
