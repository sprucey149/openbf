# B20 result: splitscreen in a LAN round, November 25, 2026

Branch `claude/split-b20` from private main `40712f0eb`. The work is in the game window
(`split.rs`, `events.rs`, `lan.rs`, `windowed.rs`), the game library (`netplay/`,
`local_seats.rs`, `name_tags.rs`, `render_blend.rs`, `walk/locals.rs`) and `ui` (`shell_ps2.rs`).
It follows SPLITSCREEN.md 18.2 B20 and B17's game side.

The fonts and the "Paused by <player name>" line already landed in B22 and B21. The
Game Menu names the player from `App.names`, so in a LAN round it names the player from the
session's names.

## What landed

- **Split in a LAN round.**
  - `split::seat` takes the Players-screen seat of each walk local (`App::lan_split_seats`,
    from the session's `LanLocal` seats and slots) and seats nobody itself. The session's
    slots and the bundles seat the players (`adopt_local` at `round_built`).
  - Each walk local is mapped to its own seat whatever the session's adopt order
    (`Seats::from_seats`). Seats the session left out (refused, or with no profile) hold nothing
    in the round; their pads are kept on `SplitState.left_out` and never take a waiting player's
    seat. The round's end hands those seats back to the Players screen (`party_back`).
  - `App::split_on` no longer requires `netplay.is_none()`.
  - The developer flag never splits a LAN round. Only the Players screen's party does.
  - The Splitscreen mode list's Multiplayer row is live (`SPLIT_LAN_LIVE = true`).
- **Pads.**
  - Each frame submits every local's own pad through `LocalView::frame_locals`, in the
    session's local order (`local_seats::lan_pads`). Each pad comes from `finish_pads` by slot,
    with that player's own viewpoint stamped.
  - One local still goes through `frame`, exactly as before.
  - Under one player's Game Menu, the other players on this machine play on. Only the menu's
    driver (and the keyboard, when it drives the menu) stands still.
- **Respawn for a further player.** A further player's Game Menu Respawn is asked of their own
  view. `lan_pads` takes it with `take_respawn_of` and puts RESPAWN in their pad once. The
  session's own Respawn handling is skipped for that action.
- **Feeds, rumble and medals.** On the LAN branch, each further player's feed reads their
  unit for messages and shows medal lines, and their own pad rumbles.
  - Nothing there writes the sim. `netplay::score` already scores every human.
- **Names.** In a LAN round the session's names stay in `App.names`; `name_split_players` no
  longer overrides them. The window lends the seat profiles when a LAN round loads
  (`lend_split_profiles`), so each player has their own view, pads and round record.
- **Drawing.** Each further view's camera and draw lists use the predicted pose and the look
  folded over that player's pending inputs:
  - the camera through `render_blend::view_camera_with`;
  - the draw lists through `ViewParts.lan_view` (`draw_in_local` and `draw_out`).
- **Name tags.** Each cell's tags come from the new `name_tags::cell_tags`. The window and the
  capture share this one function. Under the Name tags row, each cell tags the remote players
  and the other player on this machine.
- **Lost pads (B17's game side).**
  - In a LAN round, `seat_split_pads` calls only `local_seats::lan_presence`, which calls
    `Peer::set_local_away`. That sends `HostCommand::SetLocalAway` or
    `ClientCommand::SetLocalAway` through the new `Link::set_local_away`. Nothing suspends,
    resumes or installs a profile.
  - It is asked every frame and sends only changes. A new peer (a new link) starts with no pad
    away, so it is told again about every pad still missing. A guest peer records the flag
    before its `Welcome`.
- **Same side, same career.** Every `RemoveHuman` now records the slot's side, `SlotKeep` and
  class-screen points in `RoundState.kept`, on every peer.
  - `NetEvent::LocalBack` now sends `AddHuman { team: TEAM_KEPT (254) }`. That seats the slot
    on its old side and restores its round career, assist and cheats (`restore_slot`). The
    points carry into the gate (`RoundState.carried`).
  - When the slot is one of several on this machine, its kills, deaths and side are also
    banked in its round carry (presentation only).
  - `TEAM_AUTO` keeps the old rule.
- **Line-up changes.** If a running round's local line-up changes (`LocalsChanged`), the window
  lays the split out again for the new locals (`relay_lan_split`): each player who stays keeps
  their seat, pad, input and feed, and a newcomer gets their own view and FOV. With no party to
  split from, the further locals are marked away so bots hold their slots.
- **Soak.** `game --net-soak --locals N` (with `--away-at` and `--back-at`):
  - guest 1's machine seats N players, each with its own scripted pad;
  - its second player's pad goes away and comes back through `Peer::set_local_away`;
  - the soak fails unless that player is taken out, seated again and back on the same side.

## Checks

- **New tests, all passing:**
  - `netplay::tests::netplay_a_kept_return_has_its_old_side_and_round_career`: the same side,
    the same career with its progress, and the points carried. `TEAM_AUTO` still installs a
    fresh career.
  - `netplay::tests::netplay_a_guest_locals_pad_goes_and_comes_back_through_the_session_only`.
    This is the B17/B20 check, run through the window's `lan_presence` over a guest's `Seats`.
    - The only commands for the slot are one `RemoveHuman` and one `AddHuman`.
    - The player comes back on the same side with the same career, on the host and the guest.
    - The other local plays on, and the queues stay equal.
    - After a reconnect with the pad still unplugged, the slot stays out and is never seated.
    - 0 hash mismatches and 0 desyncs.
  - `netplay::tests::netplay_lan_pads_carry_each_locals_pad_and_its_respawn`.
- **Filtered runs:**
  - `cargo test -p game --lib -- netplay:: walk::locals local_seats name_tags`: 125 passed,
    2 ignored. This includes B18's two-local lockstep test and the host-local test, both still
    passing with `TEAM_KEPT`.
  - `cargo test -p game --bin game -- split:: windowed::lan windowed::lobby`: 31 passed,
    1 ignored.
  - `cargo test -p ui --lib -- shell_ps2`: 33 passed.
- **Soak with several players on one machine** (`--ticks 2500 --seed 1`):
  - `--locals 2`: 2502 ticks, 8698 hash checks, 0 mismatches, 0 desyncs, no failures. Slot 3
    was taken out and seated again, on team 1 before and after.
  - `--locals 3`: the same result.
- **One-player gate.** `cargo build --release -p game`, then `scripts/one_player_gate.py`:
  every row matches. The pinned one-player soak did not move:
  `lan_soak ticks 1559, hash_checks 4312, static_digest 9d61d689cd3e3719`.
- **Capture.** `cargo test -p game --test split_capture -- --ignored --test-threads=1
  split_lan_name_tags_capture` wrote `b20_lan_tags_1920x1080.png`, and I looked at it.
  - The setup is a LAN round's sim (`install_humans` with three humans, the second local
    adopted), side by side on HOT1, with tags made through `cell_tags`.
  - Luke's cell shows Lando (remote, red) and Leia (the other local, blue).
  - Leia's cell shows Lando only (Luke stands behind her).
  - All tags are in the HUD font.

## Review fixes

All findings from the B20 review were checked against the code and were real. Each was fixed:

- **A refused seat's pad.** It could take a waiting player's seat after another pad dropped. It
  is now on `left_out` and the offer loop skips it.
- **Game Menu in a LAN round.** The menu is drawn in its opener's cell only
  (`SplitPause.in_view`, `App::lan_menu_view`). The other cells keep their HUD, name tags, timer,
  banner, lost-pad and waiting notices, and the mouse still aims for a keyboard player who did
  not open the menu. Mouse clicks and the wheel hit-test in the opener's cell.
- **Per-player Game Menu rows.** In a LAN round they now act as the opener
  (`NetplaySession::opener_pause_action`). Pad look, assist and awards are saved to the opener's
  own lent profile, and `SetCareerAwards` goes to the opener's slot. P1's profile is not touched.
- **Round record seats.** With one view in a LAN round, the record and the career flush use the
  session's seats (`round_seat_numbers`), not 0..n. The flush also runs then.
- **Line-up change.** No longer drops to one view (see above). `seat_lan` no longer gives up when
  the adopt order differs from seat order.
- **`keep_removed`.** The side is taken only when the slot's handle is live. A stale handle
  keeps the career and lets the side be picked as `TEAM_AUTO` does.
- **Esc with nobody on the keyboard.** In a LAN round only the keyboard drives that menu.
  P1's pad stays live and is not held for resume.
- **Further players' profiles across rounds.** At the round's end, each further local's session
  profile is refreshed from their lent seat profile (`refresh_local_profiles`: medals, class
  unlocks, assist, awards row). The host seats its own locals from that. A guest sends a
  `ClientCommand::ReadyLocal` per further local (the net layer already had it), so the host
  seats them on their own profiles next round. (The host already remembered every guest
  slot's medals; unlock kills and settings were the stale part.)
- **Shot sounds.** Every local's predicted shots are played at once, and each is muted in the
  confirmed bundle (`TickHook::tick` takes the per-local bits; `SoldierVisuals::mute_shots`).
- **Refused seat notice.** The battle's feed says "<name> could not join this game" for each
  seat the session left out.

New checks:

- `windowed::split::tests::a_lan_round_seats_each_local_by_seat_and_sets_left_out_pads_aside`
- `windowed::lan::tests::lan_a_further_players_game_menu_rows_are_their_own`
- `input` `owners::tests::picked_seats_players_in_the_given_order_and_take_seat_from_fills_a_free_seat`
- Capture `b20_lan_menu_in_cell` (in `split_frames_capture`). I looked at it: "Paused by Han" in
  P2's cell only, and P1's HUD, reticle and minimap still up.

Re-run after the fixes: the filtered lib, bin, `input` and `ui` tests all pass. The one-player
gate matches every row (`lan_soak ticks 1559, hash_checks 4312, static_digest
9d61d689cd3e3719`). `--net-soak --locals 2 --ticks 2500 --seed 1`: 2502 ticks, 8698 hash checks,
0 mismatches, 0 desyncs.

## Deviations

- **The capture is not a full LAN round.** It builds the LAN round's sim and tag path directly.
  It does not run a networked round in the window: the test crate cannot reach the binary's
  round builder.
- **New code the plan did not list:**
  - `Link::set_local_away`, `Peer::set_local_away` and `Peer::local_away`;
  - `TEAM_KEPT` and `RoundState.kept` / `carried`;
  - `WalkSim::bank_removed_local`;
  - `local_seats::lan_presence` and `lan_pads`, in the game library so the library tests can
    reach them;
  - `name_tags::cell_tags`;
  - `PadOwners::picked` / `take_seat_from`, `Seats::from_seats`.
- **"Paused by <name>" in a LAN round** still says Paused, though the round runs on under it.
- **Esc with nobody on the keyboard** puts the LAN menu in P1's cell (the keyboard has no cell).

## Left

- **The maintainer's run:** 2 + 1 players finish a round with matching hashes.
- **Host locals joining mid-session** get a split view through the relayout, but a seat that
  is not on the Players screen's party has no pad until one is pressed.
- **Two pads returning under new ids at once** still go to the lowest waiting seats. This known
  limit is unchanged.
