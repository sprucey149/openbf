# B22 Leftovers: build plan (splitscreen, issue #194)

Base: private main `af4cc0787` (on top of `6dc4fc0d4`). Paths are relative to `engine/`.
Standing rules apply: KB/M never forced on a player; seats never renumber or swap; a lost pad touches only its own cell; one-player path bit-identical; no real people's names in code or fixtures (use "Han", "Leia", "Rex").

One-player gate for every item: `cargo test -p ui -- hud_frame_identity draw_hud_in_tests` (several filters go after `--`) and `cargo test -p game name_tags`. Run only filtered tests, no full suites.

---

## 0. What "plain font" is (check this first, ~15 min)

The text draws in three ways now:

| Path | Font | Used by |
|---|---|---|
| Native HUD tree (`ui/src/hud_draw.rs:1167` MultiLineText, `hud_text.rs:113` Text: `art.fonts.get(&e.font)`) | BF2 HUD bitmap `gamefont_tiny/small/large` (SDF) | kill feed, award lines, spectate and leaving lines (`ui/src/hud.rs` `add_spectate_lines`, `add_leaving_line`) |
| Native shell/menu (`Ctx::face(role)` → `HudArt::menu_font`, `hud_draw.rs:103`) | menu face `menu_gamefont_*`, BF2's own font | Game Menu, Players screen, stats pages, spawn screens |
| egui (`ui::typography::egui_font`, `typography.rs:408`) | menu face traced into egui (`typography/face.rs::install`, installed at `game/src/main/windowed/events.rs:120`); egui's own font if not installed | one-player round timer (`ui::hud::draw_hud`), LAN and split name tags (`game/src/name_tags.rs::draw_name_tags_in`, LIST) |
| egui `FontId::proportional(size)` with no role | the installed menu face by default, but sized by the cell rather than by a role | split lost notice (`game/src/split_ui.rs::draw_lost_notice`), waiting line (`split_ui.rs::draw_waiting_line`) |

Two things look wrong:
- **The split captures never install the menu face.** `game/tests/split_capture.rs::capture()` creates `EguiOverlay::headless` without calling `ui::typography::face::install(overlay.ctx(), &game::hud_feed::menu_face(&baked_root))`. Every egui text in the captures is egui's default font. Fix this first so the captures show what the game shows.
- **In game, the name tags use the menu face, while the kill feed lines next to them use the BF2 HUD bitmap font.** This is the mismatch he sees in battle.

**Correction to the earlier "known limit"** (tell him in the playtest checklist): in game, the per-cell round timer already uses the one-player `draw_hud_in` path with the menu face installed (`events.rs:118-122`). Only the headless captures lacked the face. The timer will not change. What he will see change is the name tags (now the kill feed's font) and the lost and waiting lines.

## 1. Fonts

The rule: a split-only text uses whatever the same kind of text uses elsewhere in the game.
- **Over the world** (name tags): use the HUD bitmap font, the same font and scale as the kill feed in that cell.
- **On a whole-cell screen** (lost notice, waiting line): use the menu face by typography role, like the Players screen's own lost notice and the stats pages.
- **The round timer:** unchanged (see the correction above). `draw_hud_in` is already the one-player path; the one-player gate stays exact.

**Steps:**

a. `game/tests/split_capture.rs::capture()`: install the menu face (see above) before drawing.

b. Name tags go into the native HUD pass in the kill feed's font.
   - Add `HudModel::name_tags: Vec<ui::hud::NameTagDraw { at: [f32;2] (full-cell px), name, friend, alpha }>`.
   - **Where to fill it** (`game/src/main/windowed/events.rs`): move the `NameTags::update` work, both arms (split per cell, and the LAN `(None, ..)` arm), from where it is today (~2394-2456) to **before the `hud_view` match (~2211)**. There, write each cell's tags into `split_models[i]` (mutable, before `SplitUi::paint` ~2734 reads `&split_models`) and the LAN tags into the single `model` before `hv.update` (~2248). Positions come from `to_screen(name_tags::cell_view_proj(..), ..)` in the cell's own pixel frame (`split_ui::cell_frame`).
   - **Gates** (carry over the old egui call-site gates explicitly): fill `name_tags` only when the egui path would have drawn them: not when `hud_hidden`, not when `split_menu_up` (Game Menu cover), not while the spawn screen is up, not while the view is away, not at round end. Otherwise leave the field empty.
   - **LAN under the Game Menu:** with `native_pause` the HUD view runs `update_pause`, not `update`, so LAN tags now hide under the LAN Game Menu. Intended: tags hide under the Game Menu in LAN as they already do in split. Do not carry them into `update_pause`.
   - **Draw** in `ui/src/hud_draw.rs::draw_in` through a new `Ctx::name_tags`, **before the `ctx.element` loop**, so the kill feed, minimap, bars and other HUD panels stay on top. Skip the tags while `hud.scope` is shown (a sniper scope covers the world, and tags over it would read as part of the scope).
   - **Font and scale:** look up the kill feed's element by name in `hud.elems` and use its own `e.font` and `e.text_scale` (as MultiLineText does at `hud_draw.rs:1166-1169`), with `ui::hud_tree::DEFAULT_FONT` only as a fallback if the element is missing. Scale = that text scale × `f.scale()`, where the cell's frame scale already includes the HUD Size option (`hud_k`), the same as the kill feed in that cell. `Run { shadow: true, h_align: Center }`, colour from `RETICLE_COLORS`.
   - **Positions:** tags are full-cell pixels and are drawn **without `f.project()`**. In a tall cell, `Frame::cell`'s `proj` is the 4:3 band (`hud_draw.rs:427-433`), while `cell_view_proj` spans the whole cell, so `f.project` would shift tags in 3- and 4-player tall cells. (`lock_marker` is not the pattern to copy for placement; `hud_squad.rs::squad` labels and `round_end_text` are the pattern for the text run only.)
   - **Glyph coverage:** `Ctx::run` drops characters the bitmap font lacks (`filter_map`, `hud_draw.rs:1075`), where the egui path had a fallback face. Check the name keyboard's symbols page against the kill feed font; tags and feed then have the same coverage by construction. Record the result in the test.
   - Remove the egui call sites at `events.rs:2543-2563` and `lan.rs:1297 draw_name_tags`. LAN uses the same model field on its single HUD.
   - Retire `draw_name_tags_in` once nothing uses it (after the capture rewrite below), and retire the `old_draw_name_tags` reference test, which pinned the LAN egui output; replace it with native-frame tests.
   - Keep `NameTags::update` and the occlusion and fade logic in `name_tags.rs` unchanged.
   - The one-player path has no tags (the field stays empty), so its frame is unchanged.

c. Rewrite `split_name_tags_capture` (`game/tests/split_capture.rs` ~855-866): put the tags into each scene model's `HudModel::name_tags`, which the native pass draws through `capture()`'s HudView, instead of calling `draw_name_tags_in` in the overlay closure. Keep the "with vs without tags, every changed pixel is in P2's cell" check.

d. Lost notice and waiting line: swap the raw `FontId::proportional(size)` for typography roles.
   - Lost notice: the first line in `BODY` with `tint::WARNING`, the name in `TITLE`, and "Reconnect…" in `LIST` with `tint::NOTE`. This matches the Players screen's `lost_notice` (`ui/src/shell_ps2/players.rs:316`).
   - Waiting line: `LIST` with `tint::NOTE`.
   - Size them with `egui_font(ctx, role).size * cell_text_scale(cell, ..)` (`name_tags.rs:204`), so they shrink with the cell as the timer does.
   - Drop the `// typography:` exemption comments. `ui/src/typography/tests.rs` has the role lint, so check it accepts the change.

e. Check that nothing split-only is left on the egui default font:
   - `grep -rn "FontId::proportional\|FontId::monospace" crates/game/src crates/ui/src`. The only hits allowed are dev/debug overlays (fps note, bench).

**Acceptance:**
- `cargo test -p game --test split_capture -- --ignored split_name_tags_capture split_frames_capture`. Then look at `artifacts/split_captures/*name_tags*` and `*lost*`:
  - The tags are in the same font and size as the kill feed lines in the same cell.
  - The lost notice is in the same face as the Players screen's.
  - The timer is in the same face as a one-player capture's timer.
  - New scenes: 4 players at 1280x720 with HUD Size small and with HUD Size large. Tags must be readable in a quarter cell at small; if not, set a minimum glyph height for tags and record it.
- New `ui` test `name_tags_draw_in_the_hud_font`:
  - a frame with one tag has glyph quads from the kill feed element's font page and sits inside the cell's box;
  - a tall-cell case (3- or 4-player tall cell): the tag's quad sits at the expected full-cell pixel (not shifted by the 4:3 band);
  - a name with characters from the name keyboard's symbols page: every character draws a quad, or the test records which are dropped (same coverage as the kill feed).
- New `game` test (LAN path): a frame with one tag has glyph quads after `hv.update`.
- New `game` test: a paused split frame (`split_menu_up`) has no tag quads.
- The `hud_frame_identity` tests pass unchanged.

## 2. Player names instead of P#

- **"Paused by":** `game/src/main/windowed/events.rs:858-865`.
  - Pull the `paused_by` computation out of the event loop into a small function (on `App`, or a free fn taking what it needs) so a test can call it.
  - Use `format!("Paused by {}", self.local_display_name(o))` (`split.rs:790`). This is the same name the kill feed, waiting line and lost notice use (session names table first, then `local_name`), so the pause line matches the feed for the same player.
  - Keep "Paused (keyboard)".
  - Update the comment at `windowed.rs:836`.
  - Known case: local 0 sitting on a seat other than P1 shows "Player N" (as `local_name` does at `split.rs:445`) unless the session names table has a name. Intended; record it next to the `seat_settings` note in item 4.
- **Splitscreen Options rows:** `ui/src/menu.rs:1653` `Row::SplitScreen(seat)`.
  - Add `MenuModel::split_names: [Option<String>; 4]`, by seat. Keep it off `SplitMenu`, which is `Copy`.
  - Fill it **each frame the model is built** (the menu can stay open while a seat changes profile), in `game/src/main/windowed/menu_actions.rs::split_menu`'s caller: `Some(name)` only when the seat has a profile (seat 0: the host profile) and `single_player_name` accepts the name; else `None`. Do not use `split::seat_name`, which always returns a string ("Player N" fallback).
  - **Wording:** the row label is just the name, with the value column showing the position (Left/Right/Top Left…), so no "'s Screen" suffix (avoids names ending in s, row width and translation). With no name, the label is `"P{n}"`. Ellipsize the name to the row width.
  - Update the doc comments at `menu.rs:247, 445, 453` and `game/src/split_layout.rs:116`.
- **Players screen badges** (`ui/src/shell_ps2/players.rs:216` `tag = "P{n}"`): **keep them.** They name the region's seat position, the same number the Splitscreen rows fall back to, and the profile name is already on the card under them. Record this in `analysis/plan/SPLITSCREEN.md` 18.1.
- **Console `println!("career (P{}…")`** in `career.rs:72`: dev output, leave it.

**Acceptance:**
- `cargo test -p ui split_menu_tests`: new test `split_screen_rows_show_profile_names`, using "Han" and "Leia" (labels "Han", "Leia"), a nameless seat that shows "P3", and a long name that is ellipsized within the row.
- `cargo test -p game -- windowed::tests paused_by_names_the_opener`: seat 1's profile "Leia" opens the menu and gets `Some("Paused by Leia")`; a second case for local 0 on a seat other than P1 pins the "Player N" result.

## 3. Join and reclaim prompts

There are per-pad-type sheets already: `ui/src/input_icons.rs` (`pad_sheet(IconSet, PadButton)`, `Icons::sheet(Prompt::Accept)`) and `set_for(IconChoice::Auto, Device::Pad(family))` in `game/src/main/windowed/front/players.rs:223`.

Which presses count: a seat joins only on South (A/Cross) or Start (`front/players.rs::press`, `join: fresh & SOUTH`; `local_seats.rs:145-147` `PartyPress` join/start). Reclaiming a lost seat on the Players screen is the same: "An unseated pad's A or Start reclaims the seat it lost" (`local_seats.rs:277`). Every prompt names exactly those two buttons.

- **Open region:** nobody holds it, so the pad type may be unknown. Add `PlayersView::join_icons: Option<IconSet>`, set in `front/players.rs` (near line 203): the family's set if every connected, unseated pad is the same family, else `None`.
  - `Some(set)`: draw the Accept glyph and the Start glyph (`pad_sheet(set, PadButton::Start)`) on the help line with `help_icon_on`, with `t.join_help` ("Join").
  - `None`: the same line in text, naming the real buttons without the Xbox-only letter: `TEXT_JOIN` English becomes "Press Cross/A or Start to join". Never "a button".
  - The line's layout is the same either way (glyphs fill a fixed slot), so the open quarters do not shift as pads are plugged in (item 7's "nothing moves").
- **Lost seat on the Players screen:** the seat's `icons` are known (`PlayerCell::Seated{icons}`). Draw the Accept and Start glyphs from `icons` plus "Come back" through `help_icon_on` in `lost_notice`. `TEXT_RECLAIM` text fallback becomes "Press Cross/A or Start to come back".
- The in-battle lost notice already says "press any button". Leave it.

**Acceptance:**
- `cargo test -p ui shell_ps2::tests players_`:
  - new `join_prompt_uses_the_waiting_pads_glyph`: a DS4-family view draws the `Ds4` Accept and Start sheets in the open region;
  - mixed families draw no sheet, and the text names Cross/A and Start;
  - the help line's rect is the same with and without glyphs.

## 4. Vehicle and flyer pad layouts per seat

This is already in place for seats 1-3. `game/src/main/windowed/split.rs:953-963` builds `input.pad_map` from `seat_settings(..).gamepad_layouts` for `current_of(walk, i)`, and `flight_controls` comes from the same profile. Seat 0 uses `App.context_gamepad`, which is the host profile, and that is P1's own.

Two gaps to close:
- The `pad_stale` flag (`split.rs:100`) must be set when a seat's profile changes in battle: on re-choose from the Players screen and on reclaim to a new profile. Check that `set_seat_profile` / reclaim paths call it.
- Local player 0 on a seat other than P1 still uses the host's table, and shows "Player N" in "Paused by" (item 2). Both are intended (`seat_settings` doc). Record them together and do not change them.

**Acceptance:**
- `cargo test -p game -- windowed::tests split_seat_vehicle_layout`: a new test. Seat 1's profile has a swapped Fighter layout, and P1's is the default. With local 1 in a fighter, `split.inputs[1].pad_map` is seat 1's table, and P1's `context_gamepad` is unchanged.

## 5. Victory and defeat banner per view

The cause: `walk.round_end` is shared. Its `Result` is fixed once, from `walk.player`'s team (`game/src/walk/camera.rs:395-407`). Then each further view's `hud_model` (`events.rs:1913-1922`, `walk.with_local(i, ..)`) copies it, so `ui::hud::feed` sets `hud.banner` (`ui/src/hud.rs:591`) to P1's result in every cell.

**Fix:**
- Add `game::round_end::result_for(outcome: u8, team: u8) -> Result`.
- In the split model loop, after `hud_model(w, feed, ..)`, set `end.result = result_for(w.sim.outcome, team of w.player)` inside `with_local`, mapped like `battle.rs::round_end_model`. An away player's model is `None` (`events.rs:1913`), so no banner case is needed for them.
- Model 0 and the shared sting: when local 0 is suspended at round end, `walk.player` may be the bot's unit. In `camera.rs`, compute the shared `Result` with `result_for` from `round_carry.team` while local 0 is suspended (else `walk.player`'s team). Same team today, but no longer fragile. Model 0 uses the same.
- The round-end music and announcer stay shared (P1's). Record this as the split rule: the shared speaker plays P1's sting.

**Acceptance:**
- `cargo test -p game -- windowed::tests split_banner_per_team`: a new test where P1 is on team 0 and P2 on team 1, and team 0 wins. Model 0 has `round_end.result == 0` and model 1 has `1`.
- Capture: a new `split_frames_capture` scene, `round_end_mixed_teams`, showing VICTORY in one cell and DEFEAT in the other.

## 6. An away first player keeps none of the stand-in bot's shots, hits and captures

Already right:
- `WalkSim::record_battle` (`game/src/walk.rs:1340-1360`) takes kills and deaths from `round_carry` when the slot is vacant, and points from `carried_round_points`.
- Players 2-4: their split feeds are not ticked while away (`events.rs:1457-1463`, `!walk.is_suspended(k)`), so nothing of the bot's reaches them.
- Class kills and medals: `leave_human` (`core/src/sim/spawn/humans.rs:181`) calls `remove_slot_medal_career`, so `local_medal_counts()` is `None` while the player is away, and the bot's soldier has `player_slot None`, so `track_class_kills` (`walk.rs:1280-1284`) credits nothing. No change to `flush_career_progress`.

The real leak, local 0 only: while local 0 is away, `tick_hook.rs:208-212` still calls `feed.tick(&walk.sim, walk.player)` so the round's scores keep counting, and `walk.player` may be the bot's unit. `event_feed.rs:1228-1259` and `:1401` then add the bot's shots, hits and captures to `self.battle`, which `career.rs` records through `battle_stats()`.

**Fix:**
- When local 0 is suspended (`split.rs:887`), snapshot `self.feed.battle`'s shots, hits and captures.
- While `is_suspended(0)`, restore those three fields from the snapshot after each `feed.tick` at `tick_hook.rs:208-212` (the feed still scores the round for everyone). On resume, drop the snapshot; counts after rejoin accrue normally on top of the frozen totals.
- Leave `flush_career_progress` alone.

**Acceptance:**
- New windowed-level test `an_away_first_player_keeps_none_of_the_bots_shots` (the feed lives in the windowed layer): P1 has 3 shots, P1 is suspended, the bot then fires 10 shots, hits, and captures a post; the round is recorded; P1's profile shows shots +3, hits and captures unchanged.
- Regression case: suspend P2 the same way, P2's profile shows only P2's own shots.
- Pinning test: suspend a player, let the bot kill; the profile's `class_kills` and medals do not change.

## 7. Players screen layout

Now: two halves while one seat at most is taken, then the 2x2 grid once two are taken (`ui/src/shell_ps2/players.rs:80 four_regions`, `:154 regions`). This jumps when P2 joins, and its P1-top/P2-bottom halves disagree with the battle's default side-by-side.

**Decision: always the 2x2 grid, from entering the screen** (P1 top left, P2 top right, P3 bottom left, P4 bottom right, the same positions as the battle's default 4-cell order). Nothing moves as players join, and each open quarter shows its join prompt.

- `regions()` always returns the grid. Delete `four_regions` and its uses, including `hit()`.
- Fit: `profiles::screen_in` fits a 4:3 box by `min(h, 0.75w)`. At 16:9 a 1280x360 half and a 640x360 quarter both give the same 480x360 box, so the picker does not shrink at all going from halves to the grid; only 4:3 or taller windows shrink it. No scale floor or scroll unless a 4:3 capture shows clipping.
- Update the module doc (`players.rs:3-5`, the "does not fit a quarter cell" text) and the `regions` doc (`:149-151`), and record the decision in SPLITSCREEN.md 18.1.
- Update the existing halves assertion in `ui/src/shell_ps2/tests.rs` ~868-874 to the grid.

**Acceptance:**
- `cargo test -p ui shell_ps2::tests players_regions_stay_put`: a new test where the regions for 0, 1, 2 and 4 seated seats are identical.
- The updated test at `tests.rs` ~868 passes with the grid.
- Captures: a 1280x720 scene with one seat picking in the top-left quarter and three open quarters, and one 4:3 (1024x768) scene with four pickers; check the picker is readable and not clipped.

## Documented limit (unchanged)

Two pads returning under new ids at once still go to the lowest waiting seats in turn (`game/src/local_seats.rs:83 offer_with`). Not changed in B22; revisit only if a playtest shows the problem.

## Order and landing

1. Item 0 (capture fix)
2. Items 2, 5 and 6, which are small, logic only, and have tests
3. Item 1b-e (fonts)
4. Items 3 and 7 (Players screen)
5. Item 4 (verification test)

Finish with one set of split captures for him to look at, and add these items to the playtest checklist in the ISSUE card, including: "the timer is unchanged because it already matched the one-player timer; check that name tags now match the kill feed".

---

## Review

Accepted (all 23 findings; the plan above includes them):
- Tags filled before the `hud_view` match, written into `split_models[i]` and the LAN `model`; LAN-path test added (1b).
- Item 6 narrowed to local 0's main feed (`tick_hook.rs:208-212`); players 2-4 already skipped. Two reviewers proposed different mechanics (a viewer flag on `tick` vs a snapshot); took the snapshot-and-restore one because it leaves `EventFeed::tick` unchanged. `EventFeed::set_viewer` does not exist, so it is gone from the plan.
- `flush_career_progress` gate dropped; replaced by a pinning test, with the reason (`leave_human` removes the slot career; bot has `player_slot None`).
- Join and reclaim prompts name only A/Cross and Start, never "a button"; stable line layout; reclaim presses checked (`local_seats.rs:277`).
- "Paused by" uses `local_display_name`, pulled out into a testable function; local 0 off seat P1 case tested and recorded with item 4.
- Splitscreen rows: label is the filtered profile name or "P{n}", no "'s Screen", ellipsized, refreshed every frame. Two reviewers differed on the fallback source; chose `Option` from the profile (not `seat_name`, which never falls back to P{n}).
- `split_name_tags_capture` rewritten for the native path; `draw_name_tags_in` retired.
- Tag font and scale taken from the kill feed element, including HUD Size; glyph coverage test; readability captures at HUD Size small and large.
- Tags drawn in full-cell pixels without `f.project`; tall-cell test.
- Tags drawn before the HUD element loop, skipped under the scope; old gates stated; paused-frame test.
- Picker fit risk at 16:9 removed (same 480x360 box); existing halves test updated; single-seat and 4:3 captures; both doc comments.
- Away-player banner clause removed; shared result uses `round_carry.team` when local 0 is away.
- One-player gate command fixed (filters after `--`).
- LAN tags hiding under the Game Menu recorded as intended.
- Timer "known limit" corrected for him in the checklist.
- Optional pad-family improvement dropped from B22.

Rejected: none.
