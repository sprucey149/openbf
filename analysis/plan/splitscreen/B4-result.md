# B4 result, November 25, 2026 (branch `claude/split-b4`)

Names table and award lines per view (`SPLITSCREEN.md` 18.2 B4), on private `main` at `41f882a82`.

## What landed

- New `game::player_names::PlayerNames`: names by sim player slot, merged on `set` (a slot not
  given keeps its name, so a leaver stays named), `get`, `name_or_default` ("Player {slot+1}"),
  `iter`, `clear` and a generation that moves only on a change.
- `App.names` is the battle's one name table, with `App.names_fed` (the generation the feeds last
  copied). `App::feed_names` copies the table into `self.feed` and every `split_feeds` entry when
  the generation moved; each feed's own map stays as its cache (`EventFeed::set_player_names`
  and its tests unchanged).
- Writers:
  - single player (`install_battle`): the table is cleared for the new battle's feeds and slot 0
    gets the filtered profile name, as before;
  - split (`name_split_players(&walk)`): `battle_names(seats, slots, host, split_profiles)` keys each
    local player by `walk.local_slot_of(k)` (no longer by the local index), and `seat_name` gives
    seat 0 the host's profile and seats 1-3 their own seat profile from `App.split_profiles`,
    through the same filter as a single player's name, else "Player N" by seat number;
  - LAN (`net_names_to_feed`): the session names go into the table.
- Readers:
  - every feed line (kill, team kill, suicide, death, hero, awards) through the feeds;
  - `scoreboard_model` now names every row through `feed.unit_name`, with the roster used for
    the ping only (`scoreboard_of` is its roster-only core, used by the test);
  - the lost-controller notice: `lost_lines(name)` takes the name from `App.names` by the local's slot;
  - the LAN name tags: seated roster slots, named from `App.names`.
- Per-view award lines:
  - `EventFeed::medals_seen(walk, own)` seeds a frame's seen map from `netplay.profiles` (LAN)
    plus every local slot after the first, with the first local slot at `own` when nothing
    carried it. With one local player offline it gives the same map as before: slot 0 at the
    profile's counts.
  - `TickState.views_seen` holds one clone per further local (empty with one player).
  - Each tick of a split battle notes medals in two passes: the hook's own note for P1, then
    every extra feed's tick and `score_medal_points`, then `split::note_split_medals`, which
    notes every view (P1's `medals_seen` and each `views_seen[k-1]`) once all the points are in.
    Noting inside the per-player loop (the first version) missed a later player's medal in the
    earlier views: the frame's last tick is almost always its only one, and the next frame's
    `TickState::new` rebuilt the seen maps with the new counts, so "<P2> earned ..." never
    showed in P1's view (review fix).
  - Each player sees their own medal line and "<Name> earned <award>" for every other local or
    remote human. There is no hero mirroring (18.1 item 3).

## Tests (filtered)

- `cargo test -p game --lib -- player_names:: event_feed:: round_stats::`: 48 passed. The 5 new
  tests are:
  - `player_names::tests::{a_set_merges_and_a_leaver_keeps_his_name, an_unnamed_slot_is_player_n,
    clear_forgets_and_moves_the_generation}`;
  - `event_feed::tests::split_feeds_name_both_players_by_profile_in_kills_team_kills_and_deaths`
    (a kill, a TK and a death between "Luke" and "Han" read the same in both feeds, with no
    "Player N");
  - `event_feed::tests::p2s_medal_shows_in_p2s_feed_and_as_earned_in_p1s` (in the game's order:
    P1 notes, P2 scores, both note again; P2's feed has
    "War Hero" and no spectator line, P1's has "Han earned War Hero", neither repeats, and the
    one-player seen map is still `{0: own}`).

  The existing `lan_lines_name_the_humans_by_their_session_names`,
  `single_player_lines_name_the_human_by_his_profile`, the spectator-line tests and the
  `round_stats::` name tests pass unchanged.
- `cargo test -p game --bin game -- split:: front::`: 29 passed, 2 ignored
  (GPU). This includes:
  - the rewritten `the_notice_names_the_player` and
    `a_battle_of_seats_one_three_four_names_each_by_their_seat`;
  - the new `battle_names_key_each_player_by_their_slot` (`battle_names` uses `local_slot_of`;
    on a real walk with P2 on slot 2 the table names slot 2);
  - the new `a_later_players_medal_reaches_the_first_view_on_the_same_tick` (hook note for P1,
    P2 scores War Hero, `note_split_medals`: P1 has "Han earned ..."; rebuilt maps repeat nothing);
  - the new `each_views_scoreboard_names_the_other_player_by_profile` (view 0 lists P2 as "Han",
    and view 1 under `with_local(1)` with P2's feed lists P1 as "Luke", each with its own `me`
    row; a rename reaches both feeds and an unchanged table copies nothing).
- Gate: after `cargo build --release -p game`, `scripts/one_player_gate.py`: every row matches,
  `lan_soak` ok (4312 hash checks).
- Clippy: no warnings in the touched lines. rustfmt was run on the touched files.

## Deviations

- `PlayerNames.by_slot` maps to a `String`, not an `Entry` struct: nothing else per slot is needed yet.
- `seat_name` and `battle_names` read the seat profiles themselves (`App.split_profiles`), because
  B3 left P2-P4's names as "Player N". This is the "seat profile name" the plan's split writer
  asks for.
- `lost_lines` takes the name rather than a seat number, and reads it from `App.names`; a slot
  the table has no name for falls back to `split::local_name` (the Players screen number), not
  "Player {slot+1}".
- Local player 0 plays on the host's profile, so `local_name` names it by the host only on seat
  P1; on another seat (latent: the Party host rule keeps P1 first) it is "Player N", never that
  seat's profile name.
- The Players screen's own cards still name seats from their profiles directly (B2). Only the
  host card goes through `seat_name`.
- The LAN name tags keep the roster as the "who is seated" filter and take the name from
  `App.names`, which `net_names_to_feed` fills from the same session names each frame. The
  output is the same as before.

## Left

- B8: per-view name tags in split (`Vec<NameTags>`, reading `App.names`).
- B9: per-view scoreboard and stats pages (`score_hold` per seat, `split_stats`). The readers
  already name through the feeds.
- B20: LAN with several locals writes the session names into `App.names` on the LAN branch
  (the writer exists).
- The maintainer's check with real pads: P2's medal shows as P2's own line in P2's cell and as
  "<P2> earned ..." in P1's.
