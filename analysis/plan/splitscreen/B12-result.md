# B12 result: audio wiring, November 25, 2026

Branch `claude/split-b12`, built on private `main` at `95880f179`. Plan: `SPLITSCREEN.md` 18.1 item 6
and 18.2 B12.

## What landed

- `WalkSim::first_awake()` / `first_awake_unit()` (`walk/locals.rs`): the first local player who is
  not away, as an `Option`. It is `Some(0)` (the walk's own `player`) with one local player, and
  `None` when every local player is away, so no stale handle (possibly a bot's unit) is read.
- `event_sounds::listeners(walk)`: the first awake player's `listener` (scope anchor and the 2 m back
  offset kept), then each further awake player's ear in `more`, in local order. One local player:
  `listener(walk)` exactly. The result is kept in `WalkSim::held_ears`; with everyone away
  `listeners` returns those held ears (velocities zero) and reads no camera. With split on, `vels`
  is always set.
- `Listener::vels: Option<[Vec3; 4]>` and `Listener::nearest(at) -> (index, ear)`. `listeners`
  carries each ear's unit velocity. The ordnance hums pitch each round with the velocity of the
  ear nearest it. This closes B11's `replace:` on finding the velocity by `is_player`. With
  `vels: None` (one player, tests, referees) the old rule stays: the first `is_player` unit, and
  other ears held still.
- Window wiring: the tick hook builds the listener and the foley view once per tick
  (`tick_hook::ears_and_views`). It feeds `play_event_sounds`, the buff loops and `play_ambience`.
  The LAN prediction's own-shot feedback and the commander's call gate and voice frame use
  `listeners`.
- Foley against any view: `foley::View` gains `more` (up to 3 further views), `with_more` and
  `any`. Steps and droideka rolls sound when any awake player's view draws the soldier. Each view
  uses its own cell's aspect and widened field (`TickState::view_aspects` from `split_cells`, then
  `fit_fov_y`). With one view the test is the old expression. With everyone away the view is
  `View::blind` (draws nobody), not P1's camera.
- Spawn-picker cues per player: `TickState::selection` is one `Option<Selection>` per local index,
  each read under `with_local`, `None` while that player is away. Each awake player's picker moves
  play their own `shell_select_*` cue (2-D, as before).
- Music and voice follow `first_awake_unit`: `BattleMusic::frame` and `Commander::frame` in
  `events.rs` (both now take `Option<UnitHandle>`; with `None` they keep the last side heard), and
  the walk's per-tick voice calls in `walk.rs` (`orders`, `leaving`, `cockpit`, `loudspeakers`,
  all skipped while everyone is away).
- The leaving line follows the followed unit: `Commander` holds both teams' out-of-bounds lines and
  picks the one for the followed unit's team. The excursion edge (`was_leaving`) resets when the
  followed unit changes (the voice moving to another player, or a respawn).
- Test support: the two-locals fixture moved to `walk/test_support.rs`
  (`two_locals_walk`, `posts_walk`, `spawn_locals`), so the sound and music tests share it.

## Checks (18.2 B12 acceptance)

- `cargo test -p game --lib -- sounds:: event_sounds:: music:: walk::locals:: foley:: announce:: blackbox::tests::sound`:
  105 passed, 0 failed, 5 ignored (after the review fixes) (audio device and census). This includes every existing
  `Handed` / sound pin, unchanged, and the new tests:
  - `sounds::tests::p2s_shot_is_handed_at_near_full_gain_from_p2s_ear`: on the two-locals fixture,
    P2's shot is handed at gain > 0.95 from P2's ear, centred. P1's ear alone (300 m away) hears it
    at less than half that.
  - `sounds::tests::one_local_listeners_hand_the_same_rows_as_listener`: with one local player,
    `listeners == listener` (with `vels` `None`), and the handed weapon rows are bit-equal.
  - `music::tests::music_follows_p2_while_p1_is_suspended`: both players playing gives `cis_`
    (P1's side) tracks. With P1's controller gone, the music is `rep_` (P2's side).
  - `walk::locals::tests::first_awake_is_p2_while_p1_is_away` (now also: everyone away gives
    `None`).
  - `music::tests::music_follows_p2_while_p1_is_suspended` also checks that with everyone away the
    music keeps P2's side.
  - `announce::...::a_new_followed_unit_starts_its_own_excursion`: per-team leaving line, edge reset
    on a change of followed unit.
  - Window spawn-cue tests (`cargo test -p game --bin game -- windowed::tests::bf1_spawn_team_preview
    windowed::tests::the_teammate_view_closes windowed::tests::spawn_selection_cues windowed::split::`):
    16 passed.
  - `event_sounds::ordnance::tests::a_second_ear_pitches_with_its_own_velocity`.
- `cargo test -p game --test suite -- hailfire`: compiles (one test literal gained `vels: None`).
  1 passed, 3 failed, 1 ignored. All 3 failures are the known missing Geonosis mission in the local
  bake (B11 recorded the same failures on unmodified main). They are not caused by this change.
- One-player gate (`scripts/one_player_gate.py`, release build of this branch): every row matches,
  `lan_soak` included (1559 ticks, 4312 hash checks). Run again after the review fixes: every row
  matches.

## Deviations

- By design (one voice per machine): the per-player commander lines (orders distance, the leaving
  line, cockpit and loudspeaker asks) are the first awake player's only. While P1 is awake, P2
  crossing the boundary or entering a cockpit gets no voice line. The HUD countdown is still
  per player.

- The plan's B12 file list names `foley.rs` and the window. The foley view test also had to change
  in `foley/droideka.rs`, which runs the same drawn-soldier test.
- `Event::Command`'s order line (`event_sounds::each`, left open by B11) stays at the first ear. It
  is a 2-D-style line played at an ear. Every awake player hears it at full gain, centred in split,
  so playing it at the issuing local's ear would change nothing audible.
- The headless `SoundLog` (black box) keeps `listener(walk)`. It runs single-view only.
- Foley views use `walk.camera_of(i)` with `fit_fov_y` rather than B8's `view_cameras`. No shared
  per-frame view camera exists in the tick hook.

## Left

- The maintainer's listening check (section 11 question 4 stays open). Headphone mode waits on it.
- B13 (camera shake and rain per view) is next, then the second playtest build.
