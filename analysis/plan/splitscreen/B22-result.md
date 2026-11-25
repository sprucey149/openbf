# B22 Leftovers result, November 25, 2026

Branch `claude/split-b22`, built on private `main` at `af4cc0787`. Plan:
`analysis/plan/splitscreen/B22-plan.md` (issue #194). Every plan item landed; deviations are
listed below.

## What landed

### 0. Captures show the game's fonts
- `game/tests/split_capture.rs::capture()` installs the menu face
  (`ui::typography::face::install` with `game::hud_feed::menu_face`), as the window does. Every
  egui text in the captures is now the game's own face.
- In the game, the per-cell round timer already used the menu face. The "plain font" seen
  earlier was only in the captures. The timer is unchanged.

### 1. Fonts
- **Name tags are drawn by the HUD pass, in the kill feed's font.**
  - New `ui::hud::NameTagDraw` and `HudModel::name_tags` (carried to `Hud::name_tags` by
    `ui::hud::feed`).
  - New `Ctx::name_tags` in `ui::hud_draw::draw_in`. It runs after the scope check and before
    the element loop, so every HUD panel stays on top. It draws nothing while the sniper scope
    is up.
  - Font and scale come from the message box that takes `player1.message`
    (`Hud::message_box(ui::hud::MESSAGE_EVENT)`, which is `player1systemMessages` in the
    layout). The scale is the box's own text scale, times the Scale records of the box and its
    groups, times the frame scale. A split cell's frame scale includes the HUD Size setting.
  - The fallback is `gamefont_tiny`. The text is shadowed, centred, with its bottom on the
    anchor, in the reticle's friend or enemy colour, faded with distance.
  - Positions are full-cell pixels, never `f.project`. Glyph boxes carry
    `ui::hud_draw::NAME_TAG_BOX`.
  - The window (`events.rs`) fills `split_models[i].name_tags` for each split view, or the
    single LAN `model.name_tags`, through the new `game::name_tags::placed` before the HUD view
    is laid out.
  - Gates: tags are filled only when the egui path would have drawn them. They are left out
    with the `hud` cheat, under a split Game Menu, behind a spawn screen, for an away player and
    at round end. LAN: no spawn screen, no round end.
  - Under LAN's native Game Menu the HUD lays out the menu (`update_pause`), so tags now hide
    there too. This is intended.
  - Removed: `draw_name_tags_in`, `layer_id`, `lan::draw_name_tags`, the egui call site and
    the `old_draw_name_tags` reference test.
- **Minimum tag size (new).**
  - `[ours] ui::hud_draw::NAME_TAG_MIN_PX = 11`: a tag's line is never shorter than 11 screen
    pixels.
  - The four-view 1280x720 capture at the smallest HUD Size drew 5-pixel tags, so a floor was
    needed. The kill feed beside a tag can still be smaller.
  - measure: split capture `b22_tags_four_hud_small` (1280x720, four views, smallest HUD Size)
    drew 5-px tags. The 11 px value is ours and awaits the maintainer's playtest (checklist).
- **Lost-controller notice and waiting line** (`game::split_ui`) now use the menu face by
  typography role, sized for the cell (new `split_ui::cell_font`: the role's egui size times
  `cell_text_scale`):
  - Lost notice: the notice line in BODY with the warning tint, the name in TITLE, and
    "Reconnect…" in LIST with the note tint, as on the Players screen's own notice.
  - Waiting line: LIST with the note tint.
  - The `// typography:` exemption comments are gone.
- **Audit.** `grep FontId::proportional|FontId::monospace` over `game/src` and `ui/src` now
  hits only the credits artwork, the loading stamp and a test.

### 2. Player names instead of P#
- **"Paused by".** New `App::paused_by_line(opener)` (`split.rs`) gives `"Paused by {name}"`
  through `local_display_name`, the name the feed, the waiting line and the lost notice use.
  "Paused (keyboard)" is kept.
- **Splitscreen Options rows.**
  - New `MenuModel::split_names: [Option<String>; 4]`, rebuilt every frame the model is built.
  - In battle it comes from `App::split_seat_names`. Local player 0 on seat P1 gets the host
    name; a further player gets their seat's profile. Local player 0 on another seat gets
    `None`.
  - On the shell's Splitscreen page it comes from `App::party_seat_names` (Host, Ready or Lent
    profiles).
  - Both pass the single-player name filter.
  - The row label is the name alone, with the place in the value column. With no name it is
    "P{n}". The native panel already ends a too-wide label in an ellipsis (`Ctx::text_in`).
- **Decision: the Players screen's seat tags stay "P1".."P4".** They name the quarter's seat
  position (the number the options rows fall back to), and the profile name is on the card
  under them. This is recorded in the `players.rs` module doc.
- **Known case.** Local player 0 sitting on a seat other than P1 plays on the host profile and
  is "Player N" (`local_name`) in "Paused by". Their options row says "P{n}". This is intended,
  as with item 4.

### 3. Join and reclaim prompts
- **Open quarters.** New `PlayersView::join_icons`: the icon set of the connected pads nobody
  holds, when they are all one family (`front/players.rs`, `input::usable_ids`).
  - With a set, the join line shows that pad's Accept and Start glyphs and then "Join".
  - Without one, it shows the text, which is now "Press Cross/A or Start to join".
  - Both forms use the same line rectangle (new `Ctx::glyph_line`: glyphs sized to the role's
    text, then the text, centred together).
- **Lost seat on the Players screen.** The seat's own pad's Accept and Start glyphs and
  "Come back" (new text key `openbf.split.come_back`). A keyboard seat gets the text fallback,
  now "Press Cross/A or Start to come back".
- The in-battle lost notice still says "press any button" (left as planned).

### 4. Vehicle and flyer pad layouts per seat
- These were already per seat. The context table is now built in
  `PlayerInput::follow_context`, pulled out of `sample_split_pads` so a test can reach it.
- **`pad_stale`.** Seat profiles change in battle only through the Game Menu's controls editor,
  which already calls `relayout()`. A re-choose and a reclaim happen on the Players screen
  between battles, where `PlayerInput::new` starts with no context, so the first sample builds
  the table. No new call site was needed.
- **Intended and unchanged.** Local player 0 on another seat uses the host profile's table
  (`seat_settings`), and is named "Player N" as in item 2.

### 5. Victory and defeat banner per view
- New `game::round_end::result_for(outcome, team)` and `Result::banner()`. `battle.rs`
  `round_end_model` uses `banner()`.
- New `WalkSim::viewer_team()`: the viewed player's soldier's side, or `round_carry.team` while
  they are away.
- `follow_round_end` takes the shared result from `viewer_team()`, so an away first player's
  bot unit no longer decides it.
- Each further view's model gets its own result (`split::own_banner`, inside `with_local`).
- The round-end music and announcer stay P1's: the speakers are shared.

### 6. Away first player's shots, hits and captures
- New `EventFeed::tick_away`: it ticks as usual, then restores the viewer's shots, hits and
  captures. `tick_hook.rs` uses it while local player 0 is suspended.
- **Deviation:** the plan had a snapshot kept across frames. The snapshot here is taken around
  each tick instead. The intent is the same and no extra state is needed.
- Class kills and medals were already safe: while P1 is away the slot has no medal career.

### 7. Players screen layout
- **Decision: the 2x2 grid always**, from entering the screen. `PlayersView::four_regions` was
  removed.
- `players::regions(size)` no longer takes the view.
- At 16:9 a quarter's picker is the same size a half's was. The captures at 4:3, 16:9, 21:9 and
  1024x768 show no clipping.

## Tests run (worktree, `af4cc0787` + this branch)

- `cargo test -p ui --lib -- name_tag_tests frame_identity draw_hud_in_tests players_ split_menu_tests`: **23 passed**.
  - New tests: `name_tags_draw_in_the_hud_font`,
    `name_tags_sit_at_their_full_cell_pixel_in_a_tall_cell`,
    `name_tags_cover_the_name_keyboard`, `name_tags_hide_under_the_scope`,
    `split_screen_rows_show_profile_names`, `players_regions_stay_put`,
    `players_join_prompt_uses_the_waiting_pads_glyph`.
  - Updated: `the_players_screen_draws_its_regions_and_hits_continue` (now the grid) and
    `the_screen_rows_step_their_own_seat_and_show_its_place` ("P1").
  - `the_single_view_is_unchanged` and the other frame-identity pins pass unchanged.
- `cargo test -p game --bin game -- split:: menu_actions front:: events::`: **45 passed**
  (2 ignored).
  - New tests: `paused_by_names_the_opener`, `split_banner_per_team`,
    `an_away_first_player_keeps_none_of_the_bots_shots`, `split_seat_vehicle_layout`.
  - These live in `windowed::split::tests`, not `windowed::tests`: the helpers are there.
- `cargo test -p game --lib -- name_tags round_end split_ui`: **16 passed**.
  - `the_single_view_draws_its_tags_as_before` was replaced by
    `placed_puts_each_tag_on_its_screen`.
  - `p1s_tag_lands_in_p2s_cell_and_nowhere_in_p1s` now checks the placed tags.
- `HUD_ASSETS=... cargo test -p game --test split_hud_edges -- --ignored --test-threads=1`:
  **5 passed**.
  - New tests: `a_lan_frame_draws_its_name_tag_in_the_feed_font` (LAN path through
    `HudView::update`) and `a_paused_frame_draws_no_name_tags`.
  - Coverage with the real bake: every character on both name keyboard pages has a glyph in
    `gamefont_tiny`. None are dropped.
- `cargo test -p game --test split_capture -- --ignored --test-threads=1`: **2 passed**.
  - Looked at: `b8_tags_side_by_side`, which shows "Luke" in the feed's font at the feed's
    size.
  - Looked at: `b22_tags_four_hud_small` and `b22_tags_four_hud_full`, before and after the
    floor.
  - Looked at: `four_one_away` (the lost notice in the menu face), `b7_three_options_page`
    (rows "Luke", "Han", "Leia", and "Paused by Luke") and `round_end_mixed_teams` (VICTORY and
    DEFEAT).
  - `two_paused_by_han` and `four_paused_by_leia` were renamed from `..._p2` and `..._p3`.
- `UI_CAPTURE_OUT=... cargo test -p game --test ui_capture -- --ignored capture_players`:
  **1 passed**.
  - Looked at: `players_1_16x9`, `players_1_4x3`, `players_4_16x9` and
    `players_4pickers_1024x768`.
- One-player gate: `scripts/one_player_gate.py` against the release build gave "every row
  matches", with lan_soak ok.
- The `suite` target did not build on base `af4cc0787` either (reproduced there with
  `cargo test -p game --test suite --no-run`: five E0308 at `announcer_ps2.rs:478,596`,
  `hero_audio.rs:362,370`, `squad_orders_ps2.rs:106`, callers not updated when
  `Announcer::frame`/`BattleMusic::frame` took `Option<UnitHandle>` in `53fadc7b3`). This branch
  now wraps those five in `Some(..)`, so `suite` builds and the `name_tags` fixture in
  `bug_checks.rs` compiles. `cargo test -p game --test suite -- bugx_c_a_hit_before_death
  announcer hero_audio squad_orders`: 26 passed, 1 failed:
  `hero_audio::every_actual_hero_sound_has_decodable_content_or_an_exact_original_cut`
  (UnexpectedEof decoding a baked music take, line 137; asset content, not code this branch
  touches).
- Known failure that is not this branch's: `ui typography::menu_modules_take_text_styles_from_typography`.
  Reproduced on base `af4cc0787`: the same six lines, `hud_frame_identity.rs:185-190: a font
  name: ("gamefont_large", 22)` and so on. That file and `typography/` are unchanged by this
  branch, and nothing new from it is in the list.

## Deviations

- Name tags in a single-view LAN game are drawn only by the native HUD pass. With no HUD art
  (the egui fallback HUD, no `HudView`) LAN tags no longer draw, and they now hide under the
  sniper scope as the rest of the HUD does. Both are intended.
- An away view's tags are cleared each frame, so a returning player's tags fade in fresh.

- Decisions are recorded in this note and in the module docs, not in `SPLITSCREEN.md` 18.1:
  this packet's rule is to write no new text into SPLITSCREEN.md.
- The kill feed element is found by the event it listens to (`player1.message`), not by its
  name. This holds across layouts.
- New `NAME_TAG_MIN_PX` (see item 1).
- Item 6 takes its snapshot around each tick (see item 6).
- New windowed tests sit in `windowed::split::tests`.
- The off-P1 "Paused by Player 2" case is pinned through `local_name` with `Seats<u8>`.
  `Seats<GamepadId>` cannot be built in a test.
- A Splitscreen row's long name is ellipsized by the panel's existing `text_in`. The panel's
  width rule widens the bar for the widest label first, up to the safe width. The unit test
  checks `typography::ellipsize` on a narrow room.
- The existing `two_round_end` scene was renamed `round_end_mixed_teams`. It already had one
  result per cell; the per-view logic is covered by `split_banner_per_team`.
- A fixture's "playtest_" names in `name_tags.rs` tests became "Rex_".

## Playtest checklist additions (for the ISSUE-194 card)

- The round timer has not changed: it already matched the one-player timer. Check that name
  tags now match the kill feed lines (same font, same size), in split and in LAN.
- In four views at 720p with HUD Size smallest, tags should still be readable. Confirm the
  11 px floor (`NAME_TAG_MIN_PX`, our pick from the capture, not yet playtested).
- Tags hide under the Game Menu in LAN too.
- "Paused by <name>" uses the profile name of whoever opened the menu, and the Splitscreen
  Options rows give each player the same name (both read `App::local_display_name`).
- The Splitscreen Options rows read by player name, with the place on the right.
- The Players screen is the 2x2 grid from the start, and nothing moves as pads join. The open
  quarters show your pad's buttons when every waiting pad is the same type, else "Press
  Cross/A or Start to join".
- In a battle where the teams split (P1 on one side, P2 on the other), each cell shows its own
  VICTORY or DEFEAT.
- Unplug P1 mid-round and leave them away to the end: P1's career shots and hits should not
  grow from the bot's fire.

## Left

- Two pads returning under new ids at once still go to the lowest waiting seats
  (`Seats::offer_with`). This is a documented limit, unchanged.
- The ISSUE-194 card checklist and SPLITSCREEN.md 18.1 still need the lines above copied in
  by the integrator.
- Vehicles, flyers and space maps still need their splitscreen polish, and LAN with several
  local players still needs building (B14-B21). Neither is in this packet.
