---
id: SQACK
title: Squad orders the BF1 way: the player's command line, which teammates acknowledge with which voice lines and when, and what each order makes them do — the PS2's, matched in the port
milestone: M3
value: 5
phase: build
status: done
deps: ANNC
files: engine/crates/core/src/squad.rs,engine/crates/core/src/ai/command.rs,engine/crates/core/src/ai/brain.rs,engine/crates/core/src/sim/step.rs,engine/crates/core/src/sim/input.rs,engine/crates/game/src/main/windowed.rs,engine/crates/game/src/sounds.rs
declares:
needs:
effort:
branch: sqack
resume: landed 83ea41b8 2026-10-03
created: 2026-09-30
alpha: yes
---
## Why
The maintainer's INBOX 2026-09-30 11:40: squad orders the BF1 way — "your character gives a command, and the teammate players nearby
you acknowledge what you said and then go do whatever it is you said." With the markers gone (SQMK), voice and
behaviour are the feedback, so they must be the PS2's.

## RE done 2026-09-30 — `analysis/re-squad-orders.md` (d346555; verifier 39/8/2 of 49 folded in)
The PS2's orders are NOT the port's (§9 of the doc is the side-by-side table). In short:
- D-pad Up MOVE OUT = Left AT EASE (release), Down HOLD POSITION, Right FOLLOW ME. No Disperse.
- The player's line (`<s>_inf_com_chatter_tac_moveout|hold|follow`) plays at the press whether or not anyone hears it.
- Every friendly AI within 30 m (45 m for the release orders), 3-D, takes the order at its next think (0-0.5 s) and
  answers `ai_<s>_inf_com_chatter_tac_yes` through the voice queue; a take drawn in the last 1 s (2 s all / imp) is
  silent -> 2-3 answers for 3-5 takers.
- FOLLOW: two followers per leader (the first two to think); stand within +-3 m of him in his frame, follow past 9 m;
  released after 10 s with him beyond 20 m. HOLD: every taker, stands still, released the same way (by the LEADER's
  distance, not the anchor's). MOVE OUT / AT EASE release units ordered by this player.
- BF2's own order (aim at one unit: follow / stop, rank + 1 followers) is documented in §7 and NOT built: the maintainer asked
  for BF1's.
- PS2 logs to compare with: `engine/artifacts/lead_tmp/sqack/scenes/SUMMARY.txt` (+ `events.csv` / `pos.csv` per scene).

## Original RE brief — What to find (decomp, RAM, PCSX2 — what the PS2 does)
- **The player's line:** per command (Follow, Hold, Disperse, All clear) which line the player's character says, which
  voice set (faction, class), when, any animation. The port plays a streamed take on BF1 maps (`game/src/main/windowed.rs`
  ~78-99: CLEAR always speaks, the other three only when somebody took the order) — check it against the PS2.
- **Who answers:** how many teammates acknowledge, which (nearest N? those in the order radius that accept?), after
  what delay, which lines (the per-class squad-order sounds of USND — merge the overlap), every acknowledgement or only
  the nearest's.
- **What they do:** each order's behaviour as the PS2 runs it — units taken (radius, cap, classes), how they follow,
  hold, disperse, what "all clear" releases. Compare with `core/src/squad.rs` (D10-D14).
- Whether the PS2 draws any marker or cross in normal play (SQMK's cross default).
- BF2's twin: does BF2 have squad orders at all on the PS2? If it does, its lines too; PLAN plays BF2 rules on every map.

## Original acceptance (RE phase — met by the doc; the build's is above)
- RE doc with [V]/[O] claims, verifier run.
- A PS2 versus port comparison of a command scene: a short scripted run, each command with three teammates nearby; log
  the acknowledgement lines and their timings and the units' positions over the next 10 s. Built: the port's lines match
  by name, onset within ±0.3 s (PROVISIONAL tolerance, the builder may tighten it), the units taken match by count, and
  their 10-s end positions follow the same rule (within 2 m of the PS2's pattern relative to the player).
- ANNC RE 2026-09-30 (`analysis/re-announcer.md` §1, §4): a tool for this card's PS2 side — `lead_tmp/annc/annc_live.py` names every line a live PCSX2 battle plays, per stream slot, with the game clock. In 75 s of a BF1 NAB1 Clone Wars battle with NO command from the player, the voice slot played `cis_inf_com_chatter_tac_follow`, `_tac_yes`, `_tac_needgunner`, `rep_inf_com_chatter_tac_moveout` (the AI's own squad talk; several of these segment ids belong to unnamed twin properties such as 0x7D2923A2 on `cw_vo` — the `ai_` twins of re-sound-global.md §3), while slot 2 (`cw_tac_vo`) finished one tac line at the start. All of it goes through the one-line-at-a-time voice queue ANNC builds.
- EVMSG RE 2026-09-30 (`analysis/re-event-messages.md` §2): on the PS2 an order shows to the ordering player as ONE line of the BF2 systemMessages element (upper-left, 4 lines, fade 0.5 / hold 3.0 / fade 0.5 s) in cyan (32,223,223): Locl 0x731fd493 "Follow me", 0x0e0280e5 "Move out", 0xa0590ed7 "Hold on" (B2 `FUN_00240bf8` D:161347 / 161454 / 161558 / 161696), 0xf00699ad "Get in" (`FUN_002e1670` D:269092), 0x7e3c4096 "Get out" (`FUN_002e1ef0` D:269247); BF1 has the same five strings. No unit count in the PS2 line.
- ANNC landed 2026-10-01 (feb04a8): `core::announcer::VoiceQueue::ask` is the one voice slot (4 pending, commander ahead
  of chatter, never cut) — this card's lines go through it; `game::announce::Commander::frame` is the per-frame hook.

- Planner 2026-10-01 22:00 (from ANNC's landing report): `game/tests/announcer_ps2.rs::a_sim_battle_speaks_through_the_frame_hook` took ~177 s in a debug ci run (26 s when written; since HOLD the UTA1 battle needs 6,473 ticks for 4 post events) - about 2.5 min on EVERY ci run. This card touches the voice queue and that test file: shorten the test here (fewer post events, or script the captures) and report ci's wall time before and after. Squad lines go through `VoiceQueue::ask`.

- SQMK 2026-10-02: the in-world marker and Hold cross are dev-only now (`OPENBF_DEV=1`, `squad_vis::markers_on`); the order's text line is `event_feed::order_line` (port names + count). The BF1 2-D D-pad cross with the four order names was NOT built — it is this card's.

- EVMSG 2026-10-02 (landed 9d4dade): the order's text is now a cyan line of BF2's message box (`game::event_feed::order_line`, fed in `EventFeed::tick`). Keep it ASCII: the HUD fonts have no long dash and a missing glyph is not drawn. An identical line is held back for 4 s (`SAME_LINE_TICKS`).

## Landed 2026-10-03 (296a706 + docs 83ea41b; report `reports/20261003_0330_card_SQACK.md`)
Built: the four orders on the PS2's D-pad arms, pending until each unit's think, the answers through ANNC's voice
queue with the take lock, FOLLOW (2, approach / follow / stand), HOLD (stand still), the 20 m / 10 s release, the
release orders, the text line without a count. Acceptance run: `cargo test -p game --test squad_orders_ps2 -- --nocapture`
(table in `analysis/re-squad-orders.md` 9b). Not built, with their cards: the HUD cross (SQHUD), seat orders and
boarding with the leader (SQVEH). For a cleanse: `CommandKind::Disperse`, `squad::DISPERSE_TICKS` and the commander's
spread rule (`core/src/commander.rs`, its `disperse_world` tests, `nav_stats` relax counter) are unreachable from any
pad now. The planner's note on `announcer_ps2.rs::a_sim_battle_speaks_through_the_frame_hook` is stale: it takes
0.58 s (1,790 ticks) on this branch and ci.sh 27.5 s wall with a warm target; nothing was shortened.
