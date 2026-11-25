# B13 result: camera shake and rain per view, November 25, 2026

Branch `claude/split-b13`, built on private `main` at `8b8251e99`. Plan: `SPLITSCREEN.md` 18.1
item 6 and 18.2 B13.

## What landed

- Camera shake per local player (`game/src/fx.rs`):
  - `Fx::shake` stays the first player's, untouched. A private `more_shakes` holds one
    `CameraShake` for each further local player. `Fx::shake_of(i)` reads player `i`'s shake.
  - `shake_player` collects the tick's blasts once and measures each one from every local
    player's controlled object: the vehicle when seated, otherwise the soldier, each with its
    blast sphere. Every shake then ticks once.
  - An away player (`is_suspended`, only checked with 2+ locals) takes no blast and only decays,
    because a bot may hold their handle.
  - With one local player, this is the old code path: same unit, same blasts, same tick, and no
    extra shake.
- `game::render_blend::view_camera(walk, i, cell, look, shake: Option<&CameraShake>)` applies the
  shake before `fit_fov_y`, the same order as view 0. `events.rs` passes `fx.shake_of(i)` for
  each view after the first, so the draw lists and the name tags both see the shaken camera.
- Rain (`game/src/weather.rs`):
  - `Weather::frame_draws_sized(r, camera, camvel, [w, h])`. `frame_draws` now calls it with
    `r.size()`.
  - In split, view 0's draws (`events.rs`) and each further view's draws (`split.rs`
    `build_view`) are sized to their own cell, not to the window.
- Splashes:
  - New `Weather::view_splashes(dt, views, ground)`, `views` keyed by local player index: for
    each view in turn, it runs that view's `look()` and then its `splashes()`, with the rate
    divided by the view count. Each view has its own splash clock (`view_timers`), so a view
    whose footprint shrinks (looking down) is not starved by another view draining a shared
    clock. The single view keeps the old `splash_timer`.
  - The window's `rain_splashes` (`main/windowed.rs`) takes `cells`. In split it builds one
    `View` per awake local player (`camera_of(i)` at the cell's aspect with the field widened by
    `fit_fov_y`, as drawn; P1 first) and calls
    `view_splashes`. Without split it keeps the old single `splashes` call, which reads the
    previous frame's look.

## Tests run

- `cargo test -p game --lib -- fx::tests::split_shake weather::split_tests camera_shake:: --include-ignored --test-threads=1`:
  10 passed. That is 7 new tests plus the 3 existing `camera_shake::` pins.
  - `a_blast_beside_p2_shakes_p2_and_leaves_p1_alone`, which also checks the reverse case.
  - `one_players_shake_sequence_is_unchanged`: over 60 ticks of blasts, `shake_of(0)` equals a
    `CameraShake` fed the old way, and `shake_of(1)` is `None`.
  - `an_away_player_takes_no_shake`.
  - `p2s_view_camera_carries_p2s_shake`.
  - `each_views_splashes_read_its_own_look`: P1 is 1 km away and cannot see a roof; P2 stands by
    it. None of P2's splashes land under the roof. A control that runs every look first does put
    splashes under it.
  - `two_views_share_one_rate`.
  - `a_view_looking_down_keeps_its_share` (review fix): P1 looks down, P2 level, dt 1/240; each
    gets its own footprint's count within 2. The control (one shared clock) starves P1.
  - `frame_draws_is_frame_draws_sized_at_the_renderer_size`: a headless GPU test, `#[ignore]` by
    default; run serially here.
- `cargo test -p game --lib -- fx::tests::split_shake weather::split_tests name_tags:: render_blend::`:
  32 passed (the `view_camera` signature change).
- `cargo test -p game --bin game -- split::`: 13 passed.
- One-player gate (`scripts/one_player_gate.py`, release build of this branch): every row
  matches; `lan_soak` ok, 1559 ticks, 4312 hash checks.
- rustfmt check on the touched files: clean.
- Review fixes (per-view splash clocks, `fit_fov_y` on the splash views):
  `cargo test -p game --lib -- weather::split_tests` 3 passed (1 GPU test ignored);
  `cargo check -p game --bin game` clean. The gate was not re-run: the one-player path
  (`splashes` on `splash_timer`, the `None` branch of `rain_splashes`) is unchanged.

## Deviations

- The plan's `Fx::shakes: Vec<CameraShake>` became `shake` (index 0, unchanged) plus a private
  `more_shakes` behind `shake_of(i)`. This keeps the one-player field, and its single caller,
  byte-for-byte as they were.
- `view_camera` takes `Option<&CameraShake>`, not `&CameraShake`. `None` is used by B8's
  name-tag tests and gives the unshaken camera.
- Splashes: the plan has the look run "through `frame_draws_sized`". Draws happen at render time
  and splashes at update time, so `view_splashes` runs each view's `look()` itself, right before
  that view's splashes, from the player's unblended `camera_of(i)`. The single view keeps reading
  the last render look, as before.
- No split captures were taken. B13 does not change the layout, and the capture scenes have no
  rain.

## Left

- The second playtest build (per 18.2, after B13), and the maintainer's look at shake and rain in
  split on a rainy map (Kamino).
- B14-B20 (LAN with local players), then B21 polish.
