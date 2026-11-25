# B8 result, November 25, 2026 (branch `claude/split-b8`)

Per-view egui: name tags, round timer and banner (`SPLITSCREEN.md` 18.2 B8), on private `main` at
`a5d8726a4`.

## What landed

- `game::render_blend::view_camera(walk, i, cell, look) -> Camera`: local player `i`'s camera for
  a view of `cell`. It runs the same `blend_in`, `with_local(i)`, `blend_in_looking`, `camera()`,
  `fit_fov_y(cell aspect)` and `blend_out` steps as `build_view`, so the walk is left as it was
  found.
- `events.rs`: `view_cameras: Vec<Camera>` is made once per frame, after `draw_out` (outside any
  blend) and before `overlay.begin`.
  - View 0 keeps today's shaken camera.
  - An away view gets `Camera::default()`, and nothing of that player's is read.
  - With one view it is just `[camera]`.
- `split::render_views` takes `cameras: &[Camera]`, and `build_view` takes its camera instead of
  making one inside its blend. The draw lists and the tags read the same camera. Each awake view
  1+ is still blended twice a frame: once in `view_camera` and once in `build_view` for its draw
  lists (one extra `blend_in`/`with_local`/`blend_out` per extra view per frame). The restore is
  exact (the repeatability test below).
- `App.name_tags: Vec<NameTags>`, one per local view; the single view uses `[0]`. It is reset to
  one entry at round start and resized to the view count each split frame.
  - Split, under the host profile's `settings.splitscreen.name_tags`: each view gets tags when it
    is not away, its spawn screens are not up and it is not at round end. For view 0 the spawn
    test is `spawn`; for views 1+ it is `battle::spawn_screen_up` under `with_local(k)`, the
    early-return half of `spawn_model` split out (no model is built just to test it).
  - Each view is updated from `walk.local_unit(i)` and `view_cameras[i]`, with names from
    `App.names` (`name_or_default`).
  - Each view is projected with `cell_view_proj` (that camera at the cell's aspect) and drawn into
    `cells[i]`, so placement follows `arranged`/`cell_of`, never a fixed slot.
  - Hidden views have their sight answers cleared. No tags are drawn under the Game Menu
    (`split_menu_up`, which is `split_on && native_pause`; `native_pause` already implies a menu
    there, so the cells hide exactly when the single view hides `draw_hud`) or with the `hud`
    cheat.
  - LAN single view: unchanged code path (roster filter, P1's camera at the window aspect, whole
    window).
- `game::name_tags`: `draw_name_tags_in(ctx, rect, salt, scale, tags, vp)` paints the layer
  salted by view, clipped to the cell, with the font times `scale`. `cell_text(ctx, cell, hud_k)`
  gives a cell's rect, text scale and timer top: the scale is the cell's HUD box
  (`split_ui::cell_frame(cell, hud_k).box_h`) over the window's height, so text shrinks with the
  HUD beside it and follows the HUD size setting (side-by-side 2P at 16:9: 0.67; a quarter: 0.5);
  `cell_timer_top` is `ui::hud::SPLIT_TIMER_TOP` (0.16) of that box. There are also helpers
  `cell_rect`, `cell_view_proj` and `layer_id`. `lan::draw_name_tags` is now
  `draw_name_tags_in(ctx, ctx.content_rect(), 0, 1.0, ..)`.
- `ui::hud::draw_hud_in(ctx, m, rect, salt, scale, timer_top)`: the timer and banner areas are
  `constrain_to(rect)` and anchored inside it, with salt 0 keeping the bare Ids. `draw_hud` is
  `draw_hud_in(ctx, ctx.content_rect(), 0, 1.0, 12.0)`; an anchored area aligns within its
  constrain rect, which is the content rect by default, so the output is identical. In split,
  each awake view draws its own timer/banner into its cell (salt `i + 1`, `cell_text`'s scale and
  timer top): the timer hangs under that cell's reinforcement counts (18.1 item 6), clear of the
  target-name bar. Today split drew neither.
- `split_capture.rs`: `capture` can paint an egui overlay after the last view (as the window
  does), and there is a new `split_name_tags_capture` test (see below).

## Tests run

- `cargo test -p game --lib -- name_tags:: render_blend::`: 26 passed. The 3 new tests are:
  - `to_screen_with_the_cell_aspect`: the centre lands mid-cell. A 16:9 projection would put a
    side point elsewhere. Half a 4:3 width of the unwidened field lands on the cell's edge.
  - `the_single_view_draws_its_tags_as_before`: the old LAN draw call (kept in the test as a
    reference) and `draw_name_tags_in(content_rect, 0)` give identical egui shapes at 1280x720,
    640x480 and 1280x1024.
  - `p1s_tag_lands_in_p2s_cell_and_nowhere_in_p1s`: a real `WalkSim` with two locals, with P1
    placed 15 m in front of P2's `view_camera`. It checks that the per-view `NameTags` and
    per-cell draw put "Luke" (text and shadow) inside P2's cell, clipped to it, with nothing in
    P1's cell and no self-tag. It also checks that `view_camera` is repeatable, so the walk is
    restored.
- `cargo test -p ui --lib -- hud::`: 20 passed. The 2 new tests are:
  - `draw_hud_with_salt_0_is_unchanged`: the old `draw_hud`, kept as a reference, against the new
    one; the egui shapes are identical at 640x480, 1280x720 and 1280x1024.
  - `draw_hud_in_a_cell_stays_in_the_cell`: the salted timer and banner areas are inside the
    cell and centred on it, and no unsalted area is used.
- `cargo test -p game --bin game -- split:: lan::`: 14 passed (the `render_views` /
  `build_view` signature change).
- `cargo test -p game --test split_capture -- --ignored --test-threads=1`: 2 passed. This needed
  `engine/assets_baked` and `engine/missions` symlinked into the worktree (untracked, not
  committed). `split_name_tags_capture` renders each frame twice, with and without the tag, and
  checks that every changed pixel is inside P2's cell. I looked at 1920x1080 captures for
  `b8_tags_side_by_side`, `b8_tags_top_bottom` and `b8_tags_three_swapped` (P2 at bottom left,
  blank top left):
  - "LUKE" is only in P2's cell;
  - each cell has its own 12:34 timer at its top centre;
  - the existing scenes are unchanged.
- One-player gate (release build of this branch): every row matches, and `lan_soak` is ok with
  4312 hash checks. Run again after the review fixes (`spawn_screen_up` split out of
  `spawn_model`): every row matches, `lan_soak` ok, 4312 hash checks.

Review fixes: the timer under the counts (18.1 item 6), the text scale from the cell's HUD box,
`spawn_screen_up`, `name_tags` reset per round, `split_menu_up` simplified and explained, this
note's blend count. Rerun: `name_tags::` `render_blend::` (27, new
`cell_text_follows_the_cells_hud_box`), `hud::draw_hud_in_tests` (2), `--bin game split:: lan::
death_without_a_camera` (15, the last now checks `spawn_screen_up` against `spawn_model` every
tick), `split_hud_edges --ignored` (2), `split_capture --ignored` (2; I looked at
`b8_tags_side_by_side`, `b8_tags_top_bottom` and `b8_tags_three_swapped`: each cell's timer is
under its counts, the target name bar is clear, LUKE only in P2's cell).
- rustfmt on the touched files.

## Deviations

- `view_camera` is in the library (`game::render_blend`), not the binary's `split.rs`, so the lib
  test and the capture can call it.
- It takes no `shake` argument yet. Per-view shakes (`Fx::shakes`) are B13's; until then view 0
  keeps its shaken camera and views 1+ have none, as before.
- It is called after `draw_out` instead of "before the main path's `blend_in`". Both are outside
  any blend, and view 0's HUD and camera are untouched.
- "Placement by `cell_of`": the cells already come from `arranged`, indexed by local player, so
  `cells[i]` is that player's cell.
- In split the tags apply to every view, LAN included once B20 allows split there; the name
  source is `App.names`. The roster filter stays on the LAN single-view branch only.
- The split timer's top is a share of the cell's HUD box (0.16), not read from the HUD tree each
  frame. `split_hud_edges::the_split_timer_hangs_under_the_counts` (ignored, needs the bake)
  checks it against the real BF2 layout: below `player1teams` (ends at 0.151),
  `player1targetnamegroup` (0.101) and `player1targethealthbar` (0.110), and within 0.03 of the
  box under the lowest, at four cell shapes and the three HUD sizes. The single view keeps 12
  points down.

## Left

- B13: `shakes[i]` into `view_camera`.
- B20: the LAN branch with several locals (each cell tags remote players and the co-local).
- The maintainer's look with real pads: tags across cells while moving, and the timer in 3-4
  player cells. In side-by-side 2P the timer under the counts sits on the right end of the
  system messages' first line when a long message is up (`b8_tags_side_by_side`); in the other
  layouts it clears them.
