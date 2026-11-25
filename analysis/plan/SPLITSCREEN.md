# Splitscreen design (issue #194)

Status: DRAFT 3, November 24, 2026, after three review rounds (log in section 12); round 3 found
no blockers and its edits are in. Nothing is built. Awaiting the maintainer's approval.
File cites without a directory (`submit.rs`, `bloom.rs`, `falling.rs`, `world_scale.rs`) are under
`engine/crates/render/src/frame/` or `render/src/`; reviewers re-check the paths at build time. Reviewers: [code] claims name file and line in the tree at
the time of review; [orig] claims name the analysis write-up. Add findings to section 12.

## 1. Goal and scope

Up to 4 people on one machine, in Instant Action, alone or together with LAN (#168).
Galactic Conquest does not exist; its row shows greyed out. Rise of the Empire exists for one
player (`ui/src/campaign.rs`) but not for several; its row shows greyed out in the
Splitscreen list. Training is not offered in splitscreen (the original hides it).

Not in scope: online play, a second mouse or keyboard, per-player difficulty,
mid-round joining of a new local player.

## 2. What the originals do [orig]

- Main menu: Singleplayer, Multiplayer, Splitscreen, Options. Splitscreen is dimmed while
  fewer than 2 pads are connected (`re-bf2-shell.md` rows 5-6, §3.3).
- Splitscreen opens `ifs_split_profile` (`re-bf2-menus.md` 436-446): the safe area is a top
  half and a bottom half; each half shows "Press Start Button To Join" and a profile list;
  Start launches when every joined player has a profile, then the Singleplayer mode list in
  split mode (Training hidden). The PS2 allows 2 controllers, the Xbox 4 (`iMaxControllers`).
  Four players is therefore the Xbox's own limit, not only our extension.
- Music and announcer: one listener per machine, the team of local player 0, or the
  attacking team if a second local player is on the other team (`re-announcer.md` 154).
- BF2 has horizontal-split get/set functions (symbols only, behaviour not traced,
  `a3-subsystem-map.md`). The far-sky buffer uses a 224-row scissor per view
  (`re-sky-pass.md` 229; the split path itself is untraced). Galactic Conquest keeps separate
  splitscreen saves (symbols only).
- Not found: how 3D sound is positioned for the second player. That is our decision (section 8).

## 3. Menu flow (r2)

Keep the original structure. Do not remove Multiplayer from the main menu.

```
Main Menu:  Singleplayer | Multiplayer | Splitscreen | Options | Quit
Singleplayer:  Training | Rise of the Empire | Galactic Conquest (grey) | Instant Action
Multiplayer:   Host | Join                        (one player on this machine)
Splitscreen:   Players screen ->
                 Rise of the Empire (grey) | Galactic Conquest (grey) | Instant Action | Multiplayer
                 Multiplayer -> Host | Join       (several players on this machine)
```

- The Players screen is entered only from the Splitscreen row and is shared by every mode.
  It decides who plays here; the mode below decides what is played. Nothing nests.
- Multiplayer appears twice on purpose: the main-menu row is the one-player shortcut, the
  Splitscreen list has it for local parties. Both open the same Host / Join screens; the
  lobby shows "Players on this machine: N".
- A new mode is added to the mode list in one place and works for one or many players.
- The Splitscreen row is `row(RowId::Splitscreen, false)` today, a static flag asserted in
  `ui/src/shell_ps2/tests.rs`. It needs a runtime enable: one pad or more (amendment A,
  section 15; the keyboard is always there, and joining on it happens on the Players screen the row opens). Dimming and focus skip already exist
  (`StackRow.enabled`, `tint::DIMMED_GREY`). Fix the stale doc comment at `shell_ps2.rs` 11-16.
- New `Page::Players` follows the original's layout (a cell per player showing controller,
  join prompt, profile list). It reuses `profiles::Picker`; `FrontShell.picker` is one
  `Option` today and must become one per player.
- One profile on two slots: not allowed. `Store::select` returns a bound slot, so two players
  on one profile would mean two writers of one file. A profile already taken is shown
  greyed in the other players' lists.

## 4. Input (r2)

- One keyboard-and-mouse player at most; the others need gamepads (4 gamepads is fine).
  Mouse motion arrives through the device event path (`events.rs` 233-238) and can only
  feed the K+M player.
- Today `ActivePad` follows the most recently used pad (`input/src/lib.rs` 432-490), is
  pumped in three places in `windowed.rs`, and `Kbm::pad(pad)` merges keyboard and mouse into
  it (`kbm.rs` 837). Needed: per-player `GamepadId` ownership with "most recent wins"
  switching turned off for bound pads; the K+M player reads no pad (or a pad bound to nobody
  else), so a pad user never drives the K+M unit.
- Per-player state now single on `App` and moving to per player: `gameplay_stick`,
  `stick_range`, `sprint_latch`, `resume_gate`, `view_toggle`, `score_hold`, `rumble_effect`,
  `loading_pad_blocked`, `icon_device`. Per-player settings (`controller_bindings`,
  `pad_look_sensitivity`, `flight_controls`, `viewpoint`) come from that player's profile.
- Menus: the player who opened the pause menu drives it; the keyboard and pad `menu_bits` merge
  (`windowed.rs` 2433-2440) must filter by device. Pause actions that change a player's own
  settings (sensitivity, invert, bindings) take a target player instead of editing the
  host profile (`events.rs` 1935).
- Spawn screen and class picker clicks are mouse-driven (`mouse.rs` 332-446): players 2-4 use
  pad navigation.
- Keyboard and mouse are optional (amendment A, section 15): never bound to a player unless
  someone joins on them.
- A disconnected controller affects only its player (amendment A, section 15). Hotplug of a pad that
  was never seated in this match is not supported in the first version.

## 5. Simulation and per-player state (r2)

The sim is already per slot [code]: `Sim::step(&[PadState])` is slot-indexed (`step.rs` 539),
cheats and medal careers are per slot, `target_hints` is a `Vec` grown to the player count and
`flyer_seek` walks all players. The earlier list of single-player assumptions was stale. The
remaining non-test `players[0]` uses are in `game` (`matchlog.rs` 691-727, `class_picker.rs`
276, `squad_vis.rs` 191, `rumble.rs` 190).

The real work is `WalkSim` (`walk.rs`) and what hangs off it:

| State | Today | Becomes |
|---|---|---|
| `player`, `local_slot` | one | `LocalPlayer[i]` |
| camera: `cam_*` x7, `zoom_s`, `zoom_fov_mix`, `fov_scale`, `cam_pull`, `seat_view_timer` | one | per view |
| flow: `death_cam`, `spectate`, `blend`, `class_picker`, `respawn_requested`, `backdrop`, `end_shots` | one | per player |
| presentation routines: `step_death_cam`, `step_spectate`, `notice_death` (read the sim only) | one player | looped per local player |
| `track_class_kills` / `flush_class_kills` (write profile counters only; `pending_class_kills` is one map on `WalkSim`, `walk.rs` 1223-1237) | one map, one profile | a pending map per local slot, each flushed to that player's profile |
| sim-global routines: `refill_after_side_change` (counts all humans, grows the roster), `campaign::step` | once per tick | stay a single call; never looped (looping would double-grow the roster) |
| `voice`, `music`, `minimap` | one | stay one per machine (music/announcer per section 8); minimap per view |
| `pause_menu`, `score_hold`, spawn pointer/focus, `squad_icons`, `hud_view`, `paused` on `App` | one | per player, except pause menu: one, owned by the opener |

Counts: `.player` has about 388 non-test uses in 72 files, `local_slot` 103 in 26 files. About
60-80 of them are hard (camera.rs, the death/spectate paths, `tick_hook.rs`, `music.rs`,
`blackbox.rs`, `matchlog`, `class_picker`, `pause.rs`); the rest are mechanical.

**Rule (r3): no `WalkSim` state may feed the `Sim` for a local player except through that player's
`PadState` or a session command.** Already covered: spawn, class and side choice travel in
`PadState` (`core/src/sim/input.rs` 45-62, `ScreenPick`); `request_respawn` (`walk.rs` 1124-1146) is the
only `WalkSim` flag injected into the pad. Cheats, AI cap, reinforcements and career awards have
session commands (`netplay/apply.rs` 184-250), and the splitscreen pause menu routes through
them (offline `pause.rs` 196-216 writes `sim.cheats`, `set_team_size_percent` and
`reinforcement_pct` directly). Aim assist and look sensitivity: `MenuAction::SetAssist` and
`SetPadLookSensitivity` (`pause.rs` 198-213) write the session-wide `sim.assist_intensity` and
`sim.pad_look_sensitivity`; players 2-4 set the per-slot override `slot_assist` (`core/src/sim/state.rs`
168-177, 535-540, unhashed) instead, or one player's slider changes everyone's aim. Spawn setup:
`battle.rs` 285 hardcodes `open_spawn_screens(0, ..)` and loops per local slot. Hero offers are
read only. The RESPAWN injection (`walk.rs` 1138-1146) ORs a button into
the local pad for `local_slot` only; LAN already puts RESPAWN into the input pad
(`lan.rs` 877-883). For players 2-4, respawn requests and spawn picks travel in their `PadState`.

One feed drives scoring (r3): `netplay::score` (`apply.rs` 275-286) runs `feed.positive_points` and
`heroes::scores`, which mutate the Sim. `EventFeed` is written "for ONE viewer"
(`event_feed.rs` 3, 418, `round.medal_slot` 760). Splitscreen keeps one round-driving feed: the primary local slot's, created where today's is, and
its viewer handle follows respawn (`feed.tick` indexes `sim.units[viewer.index]`, `event_feed.rs`
1122). `EventFeed::tick` is idempotent per tick and `positive_points(who)` is viewer-independent,
so the per-view feeds are presentation only and never score. Offline splitscreen needs the
same multi-slot scoring loop as `netplay::score`, which today iterates `walk.netplay.profiles`
and is the only multi-slot loop (single player scores through `heroes::tick`, crediting only
`walk.player`, `heroes.rs` 264-267): it reuses the netplay seat machinery for slots 2-4 (`install_profile`,
`gate` / `apply_slot_costs`; medal careers are hashed, `state/hash.rs` 17-27, and class unlock
costs come from `netplay.profiles`, `apply.rs` 255-266). The scoreboard model is built from
`walk.player` (`events.rs` 48) and takes a slot.

Safety gate (r2): the `original_hash_pins_without_tracing` pin steps `Sim::new(42)` and never
touches `WalkSim`, `netplay` or the `game` crate, so it cannot guard this refactor. Before
phase 1 add game-level gates that pass on current `main`:
- a recorded one-human walk replay hash on 3 maps (death and respawn, a class change, a
  vehicle, a space map);
- a LAN `virtual_net` hash;
- the existing `game/tests/suite` tests that touch the player path.
`WalkSim` mutates the sim outside `step` (`refill_after_side_change`, `campaign::step`,
`track_class_kills`, the RESPAWN injection near `walk.rs` 1138). Looping over players must keep
the one-player order exactly; humans iterate in slot order.

## 6. Rendering (r2)

Each local player gets a cell and renders the world from their camera.

- [code] The type is `Renderer`; `render_frame` is one function of about 1400 lines
  (`render/src/frame/submit.rs` 21-1428). It writes one set of shared buffers per call with
  `queue.write_buffer` (globals, points, spec, instances, grass, bolts, stars, the sun-shadow
  uniform), acquires the swapchain image once and submits and presents once. Recording several
  views into one encoder would give every view the last view's data. It also mutates state
  (`advance_sky`, `falling_draws`) and sizes bloom, filter and depth targets from the
  whole frame.
- Route: split it into a per-frame part (acquire, advance sky, present) and a per-view part,
  one submit per view. Each view renders into a cell-sized offscreen target through the
  existing `world_target` path (`submit.rs` 716-730, `world_scale.upscale`) and is blitted into
  its rectangle. This also makes bloom, filter, softness and sun flare per view with no extra
  work. Acceptance for the split: view 2 must not inherit view 1's globals (a test with two
  different cameras and lights).
- The draw list is built in one handler in `game/src/main/windowed/events.rs` (1290-1830)
  from camera singletons. Per view: soldier LOD choices (`soldier_vis/lod.rs` 171),
  `camera_see_through`, hiding the viewer's own body (`view_draws_with_hidden_instances`),
  first-person arms and gun, camera shake, effect draws, holos, cloth, live water, name tags.
  Extract `build_view_draws(walk, player)`; `fx`, `vehicle_vis`, `soldier_vis`, `deka_vis`
  and `holos` take a view. Already built per camera inside the renderer: frustum cull, point
  lights, dust, sun flare, sun-shadow cascades, sky models. Shared once: the sim, unit pose,
  `gpu.draws`. Skinned palettes are rebuilt per view unless cached per unit.
- Hazards found in round 2 (r3), all to be handled by the split:
  - `world_scale.upscale` clears the whole frame to black and draws a fullscreen triangle: view 2
    would erase view 1. It takes a viewport and `LoadOp::Load` after the first view.
  - `render_frame` takes aspect, the falling-leaf aspect and the overlay call from the window size
    (`submit.rs` 71-75, 1390; `falling.rs` 226). The new `render_view(cell_rect)` takes a cell
    size; `begin_frame` / `end_frame` own acquire, sky advance and present.
  - Bloom, particle blur and filter targets reallocate when the size changes (`bloom.rs` 168), and
    `world_scale.target` is cached by size. Rule: all cells have identical pixel sizes (the
    layout rounds down and leaves a thin border).
  - `shadows.clear()`, `frame_stats`, `acquire_ms` and the GPU timer ring (3 slots) reset per
    `render_frame`; with 4 submits per present the ring overflows. Key them per present.
  - Idempotent and fine: `falling.update`, `dust.draws`, the sun-shadow cascades (drawn in the
    encoder). No TAA or history buffers exist.
  - The HUD overlay runs once per present, or once per view with a viewport, never once per
    `render_frame`.
- Cost (not measured, reading only): total shaded pixels stay near one full screen, so four
  views are not four times the fragment work. What multiplies by the view count is draw-list
  building, culling, submission, shadow passes and the full-frame passes. Phase 0 measures
  it with `frame_profile.rs` and the GPU timer; `dyn_res` is one scale per frame today and
  would stay per frame, not per view.
- Aspect and FOV: each view uses its own aspect; horizontal FOV is kept and clamped by setting.

## 7. HUD, layout, pause menu and profile (r2)

- HUD: `HudView` mixes shared assets (art, textures, GPU pass) with per-player state
  (`hud: Hud`, `leaving`). Split it into shared `HudAssets` and a per-player `Hud`, and add a
  `viewport [x, y, w, h]` (offset plus scissor) to the paint calls; `update` and `paint`
  already lay out for a size passed by the caller. egui stays for the pause menu, scoreboard
  and lobby, on one full-screen pass; there is no per-view egui overlay. A smaller HUD preset
  at 3-4 players.
- Per-player UI rules (r3): spawn pages (`spawn_page::page(.., slot)`), squad commands and the radio
  wheel are already slot-keyed and driven by `PadState`. Keyboard-and-mouse only: spawn pointer
  clicks (`windowed.rs` 1150-1230, `kbm.set_pick`, `tap_bits`, `spawn_clicks`, `regrab_after_spawn`).
  Single for now: the 3D class preview (`spawn_preview.class_at`), `spawn_backdrop` and `can_grab`
  (`mouse.rs` 74-86, uses `walk.player` and `spawn_stage`): the keyboard-and-mouse player is always
  the primary local player. `class_picker.rs` is a developer cheat that hardcodes player
  0; it is disabled in splitscreen and not generalized.
- Layouts: 2 players side by side or top and bottom (setting, default side by side; the
  original is top and bottom); 3 and 4 players a 2x2 grid; with 3 the fourth cell is
  blank. No diagonal split.
- Position: pause menu "Swap screens" (pick a player, pick a cell).
- Pause menu: a Splitscreen section, only in splitscreen: Layout, Screen positions, HUD size,
  Controller assignment.
- Profile: `#[serde(default)] pub splitscreen: SplitscreenSettings` on `Settings`
  (`profile.rs` 128; `migrate()` and `normalize()` apply) holds layout, HUD size and cell
  preference, stored in the profile of the player in slot 1. Display, audio and graphics
  come from that same profile. Pad-to-player assignment is not saved.

## 8. Audio (r2)

[code] `crates/audio` has no listener. The game takes `Listener{pos, yaw}` from
`walk.camera()` (`event_sounds.rs` 42), computes gain and pan (`sounds/playback.rs` 269-300)
and sends one `(db, pan)` to a single stereo mix; about six call sites use `listener(walk)`;
looping voices are re-panned each tick (`play_loop_voice`, 15-44).

Decision: music and announcer follow the original (local player 0's team). 3D sounds use a
`Vec<Listener>` and the loudest listener's gain. Pan: with one shared pair of speakers a pan
from one player's yaw is wrong for the others, so a splitscreen mix uses the gain from the
nearest listener and centre pan, not a per-view pan. A sound a player's own weapon makes is
heard at full gain by that player's listener, which the max rule gives with no special case.
Phase 5 builds this and the maintainer listens before it is kept.

## 9. LAN with local players (r2, protocol change)

[code] The first draft said `net` has no one-connection-one-slot assumption. That is wrong:
- `Phase::Joined(slot)` and `Msg::Welcome` hold one slot (`net/src/host/admission.rs` 173-178);
  client state is `Connected { slot: u8 }` (`client.rs` 80, 527).
- Guest input chunks carry no slot; the host routes by the connection's slot
  (`host.rs` 540-543); `ClientCore::submit_input` takes one pad and one sequence (`client.rs` 288).
- One rejoin token per slot, slot 0 excluded (`admission.rs` 204); the host's own input goes
  to slot 0 only (`host.rs` 367-373); a seated slot with no connection is refused as an
  AddHuman (`host/round.rs` 332); `Hello` has one name and profile (`msg.rs` 81-93).
- `netplay::Peer` has one `local_slot`, used by `consumed`, the predictor, the desync report
  and `set_local_slot` (`game/src/netplay/peer.rs` 1064-1396, `view.rs` 103-148).

Change, as its own phase: bump `PROTOCOL`; `Hello` carries `local_count` and per local player
a name, profile and token (or tokens derived as `(token, idx)`); `Welcome` returns a slot
list; inputs carry a local-index prefix per pad, or one chunk carries N pads; admission
claims k slots atomically and answers `Full` if k does not fit; rejoin reclaims the set;
the host's extra local players need an input path with no connection; a departing machine
drops all its slots together (kick and leave already act on a connection). Bundles, the
scheduler and desync reports are per slot and unchanged. Cap: `MAX_PLAYERS` is counted in
humans and bounded by the bundle budget (`bundle.rs` 39, `MAX_PAD_BYTES`); 4 local players on a
16-human host fit `DEFAULT_MAX_PLAYERS`. Tests with `virtual_net`.

Also needed (r3): `Peer` has one `pending_seq`, `pending`, `consumed(local_slot)` (`peer.rs` 1395) and
one pacer, so per-local-slot queues that stay equal length under the one pacer (else `allowed` and
`pending.len()` skew). The predictor (`predict.rs` 516-527) steps other slots as
stale repeated pads and applies one local pad: decision, predict all local slots (extra cost
is small, the sim step is 1-3 ms) rather than accept confirmed-only views for them. Today `others` is filled with stale pads and only
slot `s` is overwritten, which would predict a guest's own slots 2-4 from stale input, so `others`
must exclude every local slot; the real extra work is the per-slot ledger, shots, poses and the
muted-shot path (`predict.rs` 516-534), not rollback cost. `view.rs`
look folding, `confirm_shots(w.player)` and the shot ledger become per local slot. The desync
report (`peer.rs` 1469) carries the slot set. Lobby: `ready.insert(0)`, "slot 0 is the host" and
`kick && slot != 0` (`main/windowed/lobby.rs` 858, 1357; `ui/lobby.rs` 1015) need a notion of "local to the
host": the host's extra locals are host-local slots with no connection (B16: `add_local` gives the
lowest free slot, which may lie above guests' slots, so code asks `host_local(slot)` and never
assumes slots 1-3); ready and roster treat them as the host's, and a kick of a machine drops all of its slots.

## 10. Phases and acceptance

Each phase lands alone; the one-player path stays identical, proven by the section 5 gates.

0. Evidence and gates: add the section 5 game-level gates on current `main`; measure the
   single-view GPU/CPU time by pass on 3 maps. Acceptance: gates pass on `main`; a dated note here.
1. `LocalPlayer` list in `WalkSim`, per-player input ownership, and the minimum per-player death
   and spawn flow (respawn through `PadState`), 2 players on one screen with no split (debug
   view switch). Acceptance: gates unchanged; a headless test steps 2 local humans and both
   move, fire, die and respawn.
2. Renderer: per-frame / per-view split with one submit per view, plus a "Controller disconnected"
   notice in the one cell, then `build_view_draws`, then
   per-player HUD with viewport: 2 players side by side, top and bottom, 2x2. Acceptance: a
   headless pixel-region test (cell A uniform, cell B a distinct colour, no cross-bleed, view 2
   does not inherit view 1's globals); per-view frame time with date recorded by hand;
   screenshots of 2, 3 and 4 views are for the maintainer, not an assertion. (Historical: views 2-4
   were to show a minimal HUD with no feed lines until phase 8; per-view feeds have since landed as
   `split_feeds`, `events.rs` 1288-1300, and the rest of old phase 8 is B21.)
3. Players screen (the keyboard joins in a cell, as a pad does), runtime Splitscreen row (Multiplayer inside the
   Splitscreen list stays disabled until phase 7), greyed modes, pad assignment, disconnect prompt:
   landed (`d40cfdfbe`). The per-seat profile pickers did not land; they are packet B2.
4. to 8. Replaced by amendment B (section 18): the build order is packets B1 to B21 there, each
   with its own acceptance check. The old phase numbers stay as names: phase 4 is B7 and B10,
   phase 5 is B11 and B12, phases 6 and 7 are B14 to B20, phase 8 is B21.

Tests per phase are filtered to what the phase touches (maintainer's rule). One builder per
phase; reviewers read the diff.

## 11. Open questions (round 2)

1. Per-unit skinned palette cache across views: needed, or is a rebuild per view affordable?
2. Offscreen cell targets: memory and bandwidth at 4K with 4 cells and bloom per cell. Is a
   shared bloom at the end enough?
3. The class picker, spawn page and radio / command wheel were not opened in round 1: which
   are per player and which need a keyboard-player-only rule?
4. Is "loudest listener, centre pan" good enough, or does the second player need headphone mode?
5. Is a profile taken by one slot refusing to appear for another acceptable, or do we want
   guest profiles?

## 12. Review log

Round 1, November 24, 2026 (two read-only reviewers, nothing run):
- Net (blocker): one slot per connection. Fixed in section 9, added phase 6.
- Sim audit list stale, the real work is `WalkSim` state. Fixed in section 5.
- Input needs per-player pad ownership and a K+M exclusion. Fixed in section 4.
- The hash pin is too weak to guard the refactor. Gates added to section 5 and phase 0.
- Renderer is one function writing shared buffers. Redesigned in section 6.
- HUD and egui are not per view. Fixed in section 7.
- Menu: Rise of the Empire is live, Splitscreen row is static. Fixed in sections 1, 3.
- Xbox allows 4 controllers (`re-bf2-menus.md`). Fixed in section 2.
- Audio has no listener in `crates/audio`. Rewritten in section 8.
- Pause actions edited the host profile. Fixed in sections 4, 7.

Round 2, November 24, 2026 (one read-only reviewer, nothing run; two blockers):
- Blocker: the routines table looped sim-global routines. Split into sim-global (once) and
  presentation (looped). Section 5.
- Blocker: RESPAWN and spawn picks lived in `WalkSim` flags. Rule added that a local player
  feeds the Sim only through its `PadState` or a session command. Section 5.
- Scoring feed mutates the Sim; per-view feeds are presentation only. Sections 5, 7.
- Renderer hazards (upscale clear, window-size aspect, size-keyed targets, GPU timer ring).
  Section 6.
- `class_picker.rs` is a dev cheat: disabled in splitscreen. Section 7.
- Protocol gaps: per-slot queues, prediction, desync report, lobby host-local slots. Section 9.
- Phase order: minimum death and spawn moved to phase 1; Multiplayer inside Splitscreen stays
  disabled until phase 7; criteria made headless-testable. Section 10.

Round 3, November 24, 2026 (one read-only reviewer, nothing run; no blockers):
- Rule coverage spelled out: assist and sensitivity use the per-slot `slot_assist` override, spawn
  setup loops per local slot, pause actions route through session commands. Section 5.
- The scoring feed is the primary local slot's; offline splitscreen reuses the netplay seat
  machinery and scoring loop for slots 2-4. Section 5.
- Class-kill tracking needs a pending map per local slot. Section 5.
- `can_grab` follows the keyboard-and-mouse player, who is the primary local. Section 7.
- Prediction of all local slots confirmed; `others` must exclude every local slot; per-slot queues
  stay equal length. Section 9.
- Phase 2 views 2-4 have a minimal HUD until phase 8. Section 10.
- Cite fixes (`apply.rs` 275-286, `mouse.rs` 74-86, lobby path, render path prefix).

## 13. Phase 0 results, November 24, 2026

Source: `main` at `b03ffb130` plus the bench changes of this phase. Machine: the maintainer's
laptop, load average 1.4-1.8 from other jobs (wall times are indicative; GPU timestamps move
with load, `analysis/dev-tools.md`).

Gate (`scripts/one_player_gate.py`, rows in `engine/crates/game/tests/data/one_player_gate.json`):
- Three real battle setups (`bf1_hot1` conquest, `bf2_uta1` conquest, `bf2_spa1` space assault),
  seed 1, 16 bots in total, 900 ticks, one scripted human (`OPENBF_BENCH_PLAYER=script`: spawn
  screens, walking, turning, fire bursts, jump, reload, weapon switch, action key, respawn). The
  sim hash every 30 ticks is pinned (30 distinct values per map). Each run saw the player die and
  spawn 5 times. Two runs agree; a changed row fails the gate (checked by editing a row).
- The LAN lockstep soak (`game --net-soak`, four peers on `bf1_tat2`, 1500 ticks, seed 1): no
  failure, mismatch or desync over 4312 hash checks; tick count, check count and static digest
  pinned; two runs agree.
- Not pinned by the gate, covered by the existing suites that a refactor must also keep green
  (run filtered, with data): `class_change_maps`, `walker_enter`, `vehicle_rider`, `cheat_ammo`,
  `space_assault`. The scripted human does not enter a vehicle on purpose; the action key is
  pressed every 150 ticks and whether a vehicle was in reach is not asserted. Vehicle and class
  change are therefore guarded by those suites, not by a hash row.

Single-view cost (headless bench frame: world and soldiers, no HUD or egui, no shadow-quality
setting changes; `--bench-player-cam`, 300 frames, 16 bots a side, `OPENBF_GPU_TIMING=1`):

| Map | Adapter | Size | Frame mean | GPU total | Largest GPU passes |
|---|---|---|---|---|---|
| bf1_hot1 | RTX 4000 Ada | 1280x720 | 0.30 ms | 0.08 | terrain 0.02, layers 0.02, sky 0.02 |
| bf2_uta1 | RTX 4000 Ada | 1280x720 | 0.39 ms | 0.12 | bloom 0.06, models 0.04 |
| bf2_spa1 | RTX 4000 Ada | 1280x720 | 0.74 ms | 0.15 | bloom 0.08, sky 0.05 |
| bf1_hot1 | Intel (RPL-S) iGPU | 1280x720 | 4.19 ms | 3.38 | terrain 1.44, layers 1.02, bolts+particles 0.37 |
| bf2_uta1 | Intel iGPU | 1280x720 | 4.21 ms | 3.26 | models 1.74, bloom 1.26 |
| bf2_spa1 | Intel iGPU | 1280x720 | 4.48 ms | 3.46 | bloom 1.30, models 0.89, sky 0.80 |
| bf2_uta1 | Intel iGPU | 640x360 (one quarter cell) | 2.14 ms | 1.29 | models 0.68, bloom 0.45 |
| bf2_uta1 | Intel iGPU | 1920x1080 | 4.80 ms | 3.79 | models 2.07, bloom 1.48 |

Sim tick: 0.09-0.45 ms mean, p99 under 1 ms on all three maps (1.4 % of the 33.3 ms budget at most).

What this says for the design (reading, not a splitscreen measurement):
- On the discrete GPU the frame is CPU-bound at about 0.3-0.7 ms; four views would be about 1.2-3 ms.
- On the iGPU GPU time does not fall with pixels: one 640x360 view costs 1.29 ms of GPU against
  3.26 ms for 1280x720. Four quarter views therefore cost about 5.2 ms of GPU against 3.3 ms for one
  full view (about 1.6x), because sky, bloom and model passes have a fixed part per view. Bloom
  is the biggest fixed pass (0.45 ms per quarter view), so per-view bloom is the first thing to
  share or drop in splitscreen. 1080p costs only 16 % more than 720p, so the iGPU is not
  pixel-bound here.
- Within a 16.7 ms frame, four views at 720p total on the iGPU are about 5-6 ms GPU plus about
  4 x 0.3 ms CPU for this bench's content. This is without HUD, egui, shadows set to their maximum
  and the real window loop, so it is a lower bound. Phase 2 measures the real window frame.
- Not measured: the real windowed frame (HUD, overlay, dynamic resolution), 4K, shadows by
  setting, other machines.

## 14. Phase 1 results, November 24, 2026

Source: `main` at `475394c21` plus phase 1. Deviations from section 5 are marked.

Built:
- `WalkSim::locals` (`game/src/walk/locals.rs`): each further person on the machine is a
  `LocalPlayer` (slot, unit, `LocalView`). `add_local_player` seats a human with `Sim::join_human`.
  `with_local(i, f)` runs `f` with that person's view and unit swapped in as the walk's own, so every
  existing per-player routine and reader (`camera`, `first_person`, the death camera, spectate, the
  seat view) answers for them unchanged. `step_tick` now runs the per-player part of a tick
  (`step_view`) for each local player; the voice, music, minimap and the sim-global routines stay once
  per tick. Deviation: section 5 planned a table of moved fields; the swap keeps the first player's
  fields where they are, so the 388 non-test `.player` uses did not change and the one-player path
  has no new code on it except an empty loop. The 20 per-player fields are listed once, in `LocalView`.
- Respawn: `request_respawn_of(i)` sets the RESPAWN bit in that player's own pad. RESPAWN is the
  pause menu's respawn (a suicide, `sim/step.rs` 584); a dead human returns to the spawn screens when its
  respawn timer ends. The rule of section 5 holds: a local player reaches the sim only through its pad.
- `input::owners::PadOwners` (pure, tested): up to four seats, one pad each, one keyboard-and-mouse
  player, a refused fifth, an unplugged pad keeps its seat, a pad is never rebound to another player.

Checked:
- The one-player gate is unchanged on every row (three maps and the LAN soak), on a release build of
  this phase.
- `walk::locals::tests::two_local_humans_spawn_walk_fire_die_and_respawn`: two humans on one walk spawn
  through their own screens, walk apart, fire, get their own death cameras, respawn, and a respawn press
  of one kills only that one. Plus 4 `owners` tests.
- Existing: 68 of 70 `walk::` tests pass; the 2 that fail
  (`phase_two_unobstructed_eye_is_5_m_back`, `a_ground_rider_lost_with_the_hull_holds_the_seat_view_past_four_seconds`)
  fail the same way on unmodified `main`. The suites `class_change_maps`, `walker_enter`,
  `vehicle_rider`, `cheat_ammo`, `space_assault`: 38 passed, on real data.

Not done, moved on (the acceptance of phase 1 in section 10 was the headless two-human test; these are
the parts of its text that need the window):
- The debug view switch and the window's use of `PadOwners` (routing each gilrs pad to its player,
  per-player `gameplay_stick`, `sprint_latch`, `rumble_effect`, bindings and sensitivity from each
  player's profile). They need the window loop and a person at the screen to check, and are most
  useful with the viewports of phase 2, so they go there.
- Class-kill tracking and profile counters for further players: `track_class_kills` still serves the first
  player only (a per-slot pending map is phase 3, with per-player profiles).
- Scoring and the event feed for further players (section 5, one round-driving feed): phase 2 or 8.
- The spawn-screen opening for further players at round start (`battle.rs` hardcodes slot 0): the
  test opens it with `Sim::open_spawn_screens(slot)`; the battle setup loop is phase 3.

## 15. Amendment A, November 24, 2026 (maintainer's request, not yet reviewed by a second Claude)

Two requests, both settled here. The next review round checks them with phase 2.

**The keyboard and mouse are never forced onto a player.** A computer always has them (it is how
the game is launched), and nobody should have to unplug them to play on the couch.
- `PadOwners` seats no keyboard player until someone joins on it (Enter on the Players screen).
  With only pads joined, `keyboard_player()` is `None` and the keyboard and mouse drive no soldier.
  Tested (`the_keyboard_is_never_bound_unless_someone_joins_on_it`).
- An unbound keyboard and mouse still work for menus: the Players screen, the pause menu, Esc and
  quitting. A pad-only party can be set up from the keyboard.
- The keyboard joins on the Players screen as a pad does (maintainer, November 24, 2026: the separate
  "Keyboard and mouse" line is gone): Enter seats it on the lowest free seat, its cell says
  "Keyboard and mouse", and Enter from a keyboard P1 continues as P1's A does; Esc is Back (the last
  to join leaves). Each open cell says "Press A / Start to join" and, small, "Enter: keyboard and
  mouse". The Splitscreen row is enabled with one pad or more, because the keyboard is always present
  and joins on that screen (a rule that waited for the keyboard to join first could never start).
- In a match the first local player is whoever joined first, pad or keyboard. Nothing assumes
  player 1 is the keyboard.

**A disconnected controller affects only its own screen.** Nobody else's game pauses.
- That player's cell shows "Controller disconnected" with their profile name, in that cell only
  (drawn by the per-view HUD of phase 2). The other cells play on.
- On the field a bot of their team takes over their soldier where it stands (`Sim::leave_human`, the
  call a LAN leaver uses): `WalkSim::suspend_local_player(i)`. The leave clears that slot's cheats,
  assist and medal career, so the suspend keeps them (`Sim::keep_slot`) and the resume gives them
  back (`Sim::restore_slot`, tested): a returning player keeps their medals. It also drops the
  player's death camera, spawn backdrop and seat view.
- While a player is away their unit handle may name an index a bot now holds, and 51 readers index
  `units[player.index]` without a generation check. Rule: the window skips every per-player reader
  of a suspended player (camera, HUD, minimap, black box rows, pause data) and draws the notice
  instead; `is_suspended(i)` is the one guard. The music, voice and minimap follow the first local
  player who is not suspended.
- Not covered and named: leave then rejoin is a free respawn and a free class change (accepted);
  a flyer pilot is stood out at the exit point and falls; a guided missile or droid under remote
  control is not released by the leave (to test in phase 8); an open pause menu is not closed by a
  suspend. The pause menu belongs to the lowest seated player when the unbound keyboard presses Esc.
- A takeover bot can be refused when tickets are at or below alive players; the human then simply
  vanishes with no bot (`admit`). Acceptable.
- When a pad comes back, `PadOwners::reclaim(pad)` gives it to the seat that held that pad id, else
  the lowest waiting seat, so a pad that returns under a new id still finds its player when one waits.
  This is the one way a pad the match has not seated enters it. `WalkSim::resume_local_player(i, team)`
  joins them again behind the spawn screens at no ticket cost (`Sim::join_human`): they choose a post
  and a class and are back. Tested for both players in turn
  (`a_lost_controller_leaves_a_bot_and_the_others_play_on`,
  `a_lost_pad_marks_only_its_player_and_the_pad_coming_back_clears_it`).
- A keyboard player never waits: losing their pad leaves them on the keyboard.
- LAN (phase 6): the sim half is there (`AddHuman`, `RemoveHuman` in `net/src/bundle.rs`,
  `netplay/apply.rs`), but the transport half is not: `add_human` refuses a non-zero slot with no
  connection (`host/round.rs` 332), `remove_human` runs only from connection drops, and there is no
  message for "my local player 2 left or came back". Phase 6 adds a per-slot leave and rejoin request
  from a guest, host-originated commands for connectionless slots, and tests the host's slot-0
  `RemoveHuman` path. On LAN, suspend and resume go through `apply()` (which also drops
  `netplay.profiles` and the baseline), never straight through these functions. No new sim rule.
- Window wiring (phase 2): gilrs disconnect and connect events call these functions.

## 16. Phase 2a results, November 24, 2026 (the renderer's views)

`Renderer::render_view(ViewSlot, camera, draws, skinned, clear, overlay)` draws one view of a frame
shared by several; `render_frame` is the same function with no slot, so every existing caller is
unchanged (`render/src/frame/submit.rs`, `render_frame_slot`).
- The slot gives the view's rectangle in the window. The view's world is drawn at the rectangle's
  size into its own target (the existing `world_target` path, forced on) and blitted into the
  rectangle (`WorldScale::upscale_into`: a viewport and scissor, `Clear` for the first view and `Load`
  after). Nothing else reads the window size.
- The first view acquires the window's image, a later view takes it from `held_frame`, the last
  presents. If the first view found the surface lost, the later views skip. Headless needs no hold.
- One submit per view: wgpu orders `queue.write_buffer` between submits, so each view sees its own
  globals, points, instances, grass, bolts and sun-shadow data. The sky clock advances once, in the
  first view; the GPU timer times the first view only.
- All views of one present must have the same width and height (the size-keyed bloom, filter and
  world targets are made once). The layout (section 7) rounds the cells down to equal size.
- The overlay of a view receives the view's size; the caller's pass sets the viewport to the
  rectangle (the HUD lays out for the size it is given, so no offset is needed).

Checked, headless on the dGPU (`render/tests/suite/views.rs`): two side-by-side views with different
cameras and clears fill exactly their halves (every checked pixel, both halves); four views in a 2x2
grid with different cameras show no bleed and the later views do not clear the earlier; a view's
overlay is told the view's size. The existing 91 render GPU referees and 145 render unit tests still
pass; `render_frame` has the one new line `held_frame = None` on its path.

Not done in phase 2 (2b, the game side, in this order): `build_view_draws(walk, player)` out of the
window's draw-list handler (`game/src/main/windowed/events.rs` 1290-1830) with the singletons it
uses taking a view; the window's per-view loop, `LocalView` cameras and per-player `HudView` /
`Hud` split with the overlay set to the view's rectangle; the "Controller disconnected" notice in
the one cell; the window's gilrs connect and disconnect calls; the real windowed frame-time
measurement. They need a person at the screen to check, so they follow the maintainer's say-so on
how to test.

## 17. Phase 2b fixes after the first on-screen test, November 24, 2026

The maintainer's test of `ba8dcc355` (two pads side by side, 3839x2372) found every screen off.
- Layout rule (`game::split_layout::ui_rect`): each view's UI is laid out for a virtual screen that
  fits inside its cell, never the window. A cell 4:3 or wider is its own screen (top and bottom, the
  2x2 grid); a narrower cell (two side by side in a window under 8:3) gets a 4:3 band the cell's
  width, centred, painted over the whole cell with its full-screen pieces (spawn veil, damage flash,
  round-end black) stretched to the cell (`HudView::letterbox`). A view narrower than 4:3 widens its
  vertical FOV to keep a 4:3 screen's horizontal view (`fit_fov_y`).
- Each view paints its own BF2 HUD, spawn pages, death lines and round-end screens through
  `game::split_ui::SplitUi::paint`, the one path the window and the headless captures
  (`game/tests/split_capture.rs`) share. A worker-loaded round now builds the extra players' feeds
  too (P2 had no feed, so no HUD and no spawn screen).
- Prompts follow each player's own device; the keyboard's glyphs only for the keyboard player. With
  nobody on the keyboard and mouse the pointer is hidden and never grabs.
- The native Game Menu is back in split: opened by whoever pressed Start (Esc counts as the keyboard
  player, or the lowest seated player when nobody plays on it), driven only by that player's pad
  (and the keyboard when it counts as them), drawn over the whole window with "Paused by P#", and
  its Respawn acts on that player. (November 25: the maintainer asked for "Paused by <player
  name>", the owner's profile name, instead of "P#"; packet B13b in 18.2.) Its settings rows still edit the host profile (section 4).
- The spawn screens' soldiers (the side page's soldier, the class page's line-up) stand in every
  view, each with its own `spawn_preview::Preview`, and are laid out for the view's UI rectangle,
  never the window: `split_layout::ui_view` gives the rectangle's aspect and the part of the
  camera's field it covers (in a portrait cell's band, the field before `fit_fov_y` widened it), so
  the line-up clears the class panel and the cell's edge as on a single player's screen. The mouse's
  line-up pick reads the same rectangle.
- Start on any seated player's own pad opens the Game Menu as that player (`split::seat_start_edge`;
  the active pad is only one of several busy pads). If the owner's pad goes while the menu is open,
  the menu passes to the lowest seat with a pad, else to the keyboard (`split::menu_driver`), and
  "Paused by" names the new owner. When the menu's source pad changes, buttons held on the new pad
  are not fresh presses.
- Still not split: camera shake for P2 to P4, and the rain view sized to the window.

## 18. Amendment B, November 24, 2026 (maintainer's requests after the second on-screen test)

Source: private `main` at `d40cfdfbe` (worktree at `99cc90dd0`). Not yet reviewed by a second Claude.
Cites are under `engine/crates/`. Read from the code; nothing built for this amendment.

### 18.1 Requests and decisions

**1. Every local player has their own profile.** Joining on the Players screen opens a profile
picker in that player's cell: pick one or make a new one.
- Today only `App.profile` exists, and every battle path reads it. P2-P4 have no medal career, earn
  no medal points or award bonuses, pay no class costs from their own career, and nothing is saved
  for them (`battle.rs` 299-325 seats them with `Sim::join_human` only; `heroes.rs` 264-267 credits
  `walk.player` only; `track_class_kills`, `walk.rs` 1256, runs for the first player only).
- Profile core (`game/src/profile.rs`, `profile/store.rs`):
  - `Profile::id() -> Option<u64>` from the private slot, so seats can refuse a taken profile.
  - `Store::bind(id)`: `select` without moving `catalog.last` (store.rs 149-155). `Store::create_quiet`:
    `create` (157-180) without moving `last`. Picks for P2-P4 never change which profile the title
    offers next launch. The title picker keeps `select` / `create`.
  - A seat profile that has no slot never falls through to `$OPENBF_PROFILE` in `save_with`
    (profile.rs 591-597): such a profile does not save.
  - One profile on one seat only (section 3 stands): the ids held by the other seats, P1's
    included, show greyed in a seat's list and are refused. Two holders would each write a stale
    whole entry (store.rs 34-42) and erase each other's kills and medals.
- State: `FrontShell.seat_profiles: [SeatProfile; 4]` keyed by Players-screen seat number,
  `SeatProfile = Open | Picking(Picker) | Host | Ready(Profile) | Lent(u64)`. Seat 0 holds `Host`, a marker
  that means `App.profile`; no copy of P1's `Profile` is ever kept in `seat_profiles`. In battle a
  local index reaches its profile through `Seats::seat_number(i)`.
- One live `Profile` per seat, never a clone. At launch the `Ready(Profile)` values of seats 1-3
  are moved out with `take` (the seat becomes `Lent(id)`, which greying, the title-picker refusal and Continue treat as
  `Ready`) into `App.split_profiles: [Option<Profile>; 4]` keyed by seat number, and
  `leave_battle` moves them back (`Lent` becomes `Ready(Profile)` again), as `party_owners` does for the seats. In battle, `record_round`,
  the career flush and `persist_player` (B10) all write that one `App.split_profiles` entry (seat
  0: `App.profile`). The next match or map takes them again. No `Profile` is cloned across the
  front/battle boundary.
- Host rule (option A): seat 0 is the host and its profile is always `App.profile`. Seat 0 cannot
  be unseated while any other seat is seated (B on seat 0's List is refused with the error cue
  while others sit; alone it leaves the Players screen as today). Pads and the keyboard join the
  lowest free seat, so seat 0 is never a hole while others are seated, and local 0 is always seat
  0. While seat 0 is `Picking`, `App.profile`'s id still counts as held. Every career write goes
  by seat number (local 0 included through `seat_number(0) == 0`), never through `App.profile` by
  default.
- P1: seat 0 starts as `Host`. West (X / Square) on a `Ready` / `Host` seat reopens that seat's
  picker. A new pick on seat 0 calls a new `apply_host_profile(profile)` helper split out of the
  `profile_action` success block (`front/profiles.rs` 342-374): set `self.profile`, window mode,
  `kbm.follow` and key release, context bindings, pad warning, music volume, campaign unlocks. The
  title picker keeps the rest of that block (`self.session` reset, `front.ia = None`,
  `campaign_saves.current = None`, `menus.profile_selected()` which goes to `Page::Main`,
  `front.picker = None`); the seat picker calls only the helper, so the other seats stay on the
  Players screen with the session options intact.
- Title picker (Back on Main, `front/profiles.rs` 188-196): while any seat other than seat 0 is
  `Ready`, those ids are greyed and both Select and Delete refuse them, so the title picker can
  neither delete a held profile nor make `App.profile` the same id as another seat.
- Seat pickers, compared with the title picker: no Delete (delete only from the title picker, so a
  live holder never loses its entry); no Difficulty page (difficulty is the host's; `create_quiet`
  takes the host's); the name keyboard suggests "Player N" as today (`front/profiles.rs` 239-246).
- Input per seat: `PartyPress` and `poll_party_pads` (`front/players.rs` 42-79) grow d-pad, L1/R1,
  West and North with per-pad auto-repeat. The press mapping changes: Back is East only (today
  `EAST | NORTH`, `front/players.rs` 65); North has its picker meaning; South confirms inside a
  picker. Continue is Start on any seated pad (Enter for the keyboard seat) or South on a `Ready`
  / `Host` card, and neither Continues while any seat is `Picking` (today South on seat 0 always
  Continues, `local_seats.rs` 265). Each seat's `FrameInput` comes from its own pad only. The
  keyboard seat alone gets `key_menu_edges`, typed text (`profile_keyboard`, extended to the keyboard
  seat's picker) and the pointer. The pointer never drives a pad seat's picker.
- Back: B on Name returns to that picker's List; B on a `Ready` seat returns it to `Picking`; B on
  List unseats that seat only (the presser, not the highest seat; `Party::back`, `local_seats.rs`
  240, changes to take the seat, and `Seats::from_owners` already handles holes), except seat 0
  under the host rule above. Seats never renumber. Esc is the keyboard seat's B; with nobody on
  the keyboard, Esc does nothing while any pad seat is seated (only B on each pad unseats that
  pad), and leaves the Players screen only when nobody is seated. If the screen is left any
  other way, the party and seat profiles are kept for the session, as `party` is.
- A seated pad that disconnects on the Players screen keeps its seat and its `SeatProfile`
  (`Party::step` already keeps the seat, `local_seats.rs` 252-259); its region shows "Controller
  disconnected" and its picker is frozen; reclaiming the pad gives the seat back as it was. A different pad that reclaims the lost seat
  (`PadOwners::reclaim` falls back to any waiting seat) resets that seat to `Picking`, so nobody
  plays on another person's profile; the B2 check covers it. Only that region changes.
- Continue needs at least 2 seated and every seated player `Ready` / `Host` (the original's rule,
  `ifs_split_profile`).
- Drawing: the picker does not fit a 210x126 cell (`ui/src/shell_ps2/players.rs` 103-108).
  `Picker::draw` / `target` / `back_at` (`ui/src/shell_ps2/profiles.rs` 382-425) take a `Screen`
  instead of calling `screen(size)` (404). The Players screen is laid out as 2 or 4 regions, each a
  whole 512x448 virtual screen: top and bottom halves with up to 2 seated, else the 2x2 grid. Each
  region holds its seat's join prompt, picker or `Ready` card (profile name, device, career line).
- Guest (unsaved) profiles: not in this amendment (section 18.3).

**2. Name tags over heads, in splitscreen and on LAN alike.**
- `name_tags.rs` already plans one `NameTags` per view and picks human slots other than the
  viewer's; local co-players qualify with no sim change. Today tags exist only on LAN: the gate at
  `events.rs` 2027 needs `netplay`, projects from P1's camera at the window aspect and draws over
  the whole window (`lan.rs` 1297).
- Decision: `App.name_tags: Vec<NameTags>`, one per local view. For each view not away, not on the
  spawn screens and not at round end, update from that view's camera. Draw with
  `draw_name_tags_in(ctx, rect, salt, tags, vp)`: the cell in points, layer Id salted by view, a clip
  rect set to the cell, font scaled by `cell height / window height` (clamped). The old function is
  `draw_name_tags_in(ctx, content_rect, 0, ..)`, the same Ids and output as LAN today.
- The per-view cameras are needed before `overlay.begin`. `split::view_camera(walk, i, cell,
  look: &PadState, shake: &CameraShake) -> Camera` is factored out of `build_view` (`split.rs`
  ~803-817): it does its own `blend_in`, `with_local(i, blend_in_looking(w, look), camera, shake,
  fit_fov_y)`, `blend_out`, and is called only outside an active blend (before the main path's
  `blend_in`, `events.rs` 56, so view 0's HUD never reads an unblended pose). It runs once per
  view per frame; the results go in a `view_cameras: Vec<Camera>` that both the name tags and
  `build_view` read (`build_view` takes the camera instead of computing it inside its blended
  closure), so the two cannot differ and nothing is blended twice. View 0 keeps today's camera
  with shake. The projection is the cell's aspect.
- Placement follows `cell_of(player)` (18.1 item 4), never the local index.
- A "Name tags" On/Off row in the Splitscreen options (host profile, default On). A single-view
  LAN round keeps them always on (it has no Splitscreen page). In a LAN round with 2+ locals the
  row applies, to every tag in every cell, remote players included.

**3. Names everywhere multiplayer has them.**
- New `game/src/player_names.rs`: `PlayerNames { by_slot: BTreeMap<u8, Entry> }`, merged on set (a
  leaver keeps his name), `name_or_default(slot)` = "Player {slot+1}", a generation counter.
  `App.names` is the one name source.
  - Writers: single player (`windowed.rs` 1921), split (`name_split_players`, `split.rs` 362, writes
    `(walk.local_slot_of(k), seat profile name)`; this fixes `battle_names` assuming slot == k,
    `split.rs` 274), LAN (`net_names_to_feed`, `lan.rs` 1410).
  - Readers: `EventFeed::set_player_names` for `self.feed` and every `split_feeds` entry when the
    generation moves (the feed's own map stays as its cache; its API and tests stand); name tags;
    `scoreboard_model` (`events.rs` 26-49) names through `feed.unit_name` always, the roster only
    for ping; `seat_name` (`split.rs` 263) and `lost_lines`.
- Kill, team-kill, suicide, death and hero lines already run per view (`split_feeds[k].tick`); they
  only said "Player N". The table fixes that.
- Awards: today no line can appear for P2-P4 because they have no medal career. With item 1's
  careers installed: `TickState.medals_seen` becomes one map per local view, all seeded from every
  local slot plus `netplay.profiles` (`tick_hook.rs` 34-56); the split loop adds
  `note_medals(sim, local_unit(k), local_slot_of(k), &mut seen[k])` after each extra feed's tick.
  Each player sees their own medal line and "<Name> earned <award>" for every other human, local
  or remote. Decided, no mirroring: `heroes::tick` (`heroes.rs` 264-268) writes no lines of its
  own; it is `feed.tick(sim, walk.player)`, then `score_medal_points(walk.player, points)` and the
  hero-manager scores. The extra feeds' `tick` already makes every line for their viewer; what
  is missing is that the split loop drops their points (`events.rs` 1289-1296), so B3's
  medal-point loop calls `score_medal_points(local_unit(k), points)` with them. `scores` stays
  from `self.feed` alone.
- The extra feeds' opening medals: `split::extra_feeds` (`windowed.rs` 1914) builds them with
  `walk.local_slot = 0` before `split::seat` runs (`enter_battle`, 2118), so each one's
  `round.medal_slot` and `opening_medals` are P1's (`event_feed.rs` 759-761, read by
  `round_stats.rs` 364). B3, after each extra player's career is installed, calls
  `walk.with_local(k, |w| split_feeds[k-1].set_opening_medals(w))`, as `windowed.rs` 1875 does
  for `self.feed`.
- Scoreboard per view: `score_hold` one per seat, each from that seat's own pad (the keyboard only
  for the keyboard seat); `scoreboard_model` takes `walk.local_unit(i)` and that view's feed;
  `draw_scoreboard_in(ctx, rect, salt, b)` with the old function wrapping it (`ui/src/scoreboard.rs`
  210).
- Round-end stats per view: `stats_pages(walk as i, feed_i, career_i)` per local player
  (`battle.rs` 344-373), kept as `App.split_stats: Vec<StatsScreens>` for views 1-3 and swapped into
  that view's model; `walk.round_end` stays shared. Each cell's page cursor follows its own pad.

**4. Splitscreen options.**
- Saved on the host (P1) profile: `#[serde(default)] Settings.splitscreen: SplitscreenSettings
  { two_player: TwoPlayerSplit, cells: [u8; 4], name_tags: bool, hud_size: HudSize }`, with
  `#[serde(default)]` on the struct too, so a partly missing entry loads. `Default` is written by
  hand (a derived one would give `name_tags: false` and `cells: [0; 4]`): `two_player:
  SideBySide`, `cells: [0, 1, 2, 3]`, `name_tags: true`, `hud_size: Full`.
  `cells[seat]` is the 2x2 grid position of seat P1..P4 (0 top-left to 3 bottom-right), always a
  permutation; `normalize()` resets a bad one to the identity. Default two-player split stays side
  by side (today's), the original's top and bottom one row away. `TwoPlayer` (`split_layout.rs`
  17) does not get serde: all profiles share one `profiles.ron` and `Store::access` stops on any
  parse error (store.rs 67-80), so an unknown variant would make every profile unreadable. The
  profile owns `TwoPlayerSplit` and `HudSize` with a fallback variant (`#[serde(other)] Unknown`)
  that `normalize()` maps to the default, and maps `TwoPlayerSplit` to `TwoPlayer` itself.
- Layout: `split_layout::arranged(players, w, h, two, cells, seat_of) -> Vec<Cell>` indexed by local
  player. 3-4 players: the 2x2 grid permuted, so with 3 the blank cell can be any. 2 players: the two
  seats ranked by their `cells` values, lower gets left (or top). With the identity it equals `cells`
  for every n (unit test). `App::split_cells` (`split.rs` 453) calls it with `seats.seat_number`;
  every consumer already indexes by local player and is recomputed each frame, so a change re-lays
  out on the next frame. `SplitState.layout` (`split.rs` 201) reads the profile.
- Menu: `SettingsPage::Splitscreen` as the last variant; `MenuModel.split: Option<SplitMenu>`;
  Options appends the row only when `split` is set; rows "Two players" (Side by side / Top and
  bottom, locked with 3-4 players), one "P# screen" per seated seat (with 3-4 seated, arrows
  move that seat to the next grid position and swap with whoever or the blank held it; with 2
  seated, the row is a two-way choice Left / Right, or Top / Bottom following the Two players
  row, and any arrow on either row swaps the two seated players' `cells` values, so every press
  changes the screen and absent seats' values never matter), "Name tags", "HUD size" (Full / Smaller /
  Smallest = `hud_k` 1.0 / 0.85 / 0.7, 18.1 item 5). `ClosePage` (`pause.rs` 148-160) looks the
  parent row up in a model with `split` set, or Back lands on row 0.
- Reached from the Game Menu's Options in a split match, and before a match from a new "Splitscreen
  Options" row in the SplitModes list (`ui/src/shell_ps2.rs` 566-578, routed by `front.rs` 1050).
- Controller assignment is not here: the Players screen owns it and seats never move on their own.
- The keyboard player's mouse uses that player's own cell, never cell 0 (`windowed.rs` 1206,
  `events.rs` 690). `step_spawn_pointer` acts on the walk's own spawn stage (P1's), so for a
  keyboard player at local k it runs inside `walk.with_local(k)` with that player's spawn page
  and the k-th cell, and the `split_keyboard_first` gate (`events.rs` 501, 690, 1340) goes.

**5. HUD items at each cell's edges, no 4:3 band.**
- Today `ui_rect` (`split_layout.rs` 79-87) lays the HUD out in a centred 4:3 band and
  `HudView::letterbox` (`hud_feed.rs` 1135) moves it down; the HUD has horizontal anchoring only
  (`anchor_shift`, `ui/src/hud_draw.rs` 218) and no vertical anchoring.
- Decision: one pure helper, `ui::hud_draw::Frame`, used by every HUD, spawn, stats and round-end
  call site and every hit test. `box_h = min(h, 0.75 w) * hud_k`, `box_w = 4/3 box_h`, scale from
  `box_h`, `extra = [w - box_w, h - box_h]`; x keeps today's thirds rule on `extra[0]`; y gets Top
  (0), Centre (`extra[1]/2`) or Bottom (`extra[1]`). At 4:3 or wider with `hud_k` 1, every value is
  today's (vertex test at 640x480, 1280x720, 3840x1080). Narrower than 4:3 it is not: today
  `frame_scale` / `anchor_shift` use the height alone and `extra` goes negative (`hud_draw.rs`
  206-227). So `Frame` is used only for split cells (`local_count() > 1`); the single view keeps
  `frame_scale` / `anchor_shift`, which keeps a 1280x1024 or narrow resized one-player window
  bit-identical. Moving the single view to `Frame` too is a later choice for the maintainer.
- Vertical anchor per HUD root: the root subtree's frame-space box centre, computed once in
  `Hud::new` (below 0.4 Top, above 0.6 Bottom, else Centre), plus a small table, marked ours:
  Bottom for weaponinformation, jetpackbar, vehicleseatinggroup; Top for objectivetimerGROUP,
  Victory/DefeatTimerGroup, flags; Centre for HintPopup BorderBox, spawnmessage, spawnvehicle,
  hacking. Result: scores, minimap, objectives, target name and the feed at the top edge; health,
  energy, weapons, tooltips, lock info at the bottom edge; reticle and popups centred.
- Projected pieces (reticle, lock-ons, markers) map through `proj` = today's band relative to the
  cell, the part of the widened field the unwidened camera covers. The split HUD models take the
  band's aspect, not the cell's, through a new `hud_aspect` (band) used at `events.rs` 1627;
  `view_aspect` (`events.rs` 511-513, the cell) stays as it is for `fit_fov_y` (1689-1691), so the
  FOV still widens. This also fixes markers spread about 1.65x sideways in tall cells (inferred,
  not seen).
- Code-drawn pieces: capture icon top-right, squad cross bottom-right, hit chevrons on `box_h` with
  top and bottom chevrons at the cell edges, scope a 4:3 picture in `proj` with black above and
  below (it stretches today). Damage flash, spawn veil, round-end black: the whole cell.
- Spawn pages, stats pages and the round-end banner stay centred panels in a `box_h` screen centred
  in the cell (the `pause_draw` rule); their footers go to the cell's real bottom (`help_bottom`).
  The 3D line-up keeps `ui_view`.
- Game side: `split_ui.rs` 87-91 lays out and paints the whole cell; `letterbox` and its dy go.
  `ui_rect` / `ui_view` stay as the projection and line-up band. Changing the split or the order
  re-lays out with no extra code (item 4).
- The original's `2playerhud` trees are evidence only (they push items to each half's edges, top
  and bottom only); not loaded.

**6. The rest.**
- Camera shake P2-P4: `Fx::shakes: Vec<CameraShake>`, index 0 today's `shake` untouched;
  `shake_player` (`fx.rs` 1391) collects blasts once, measures from each awake local player and
  clears after all; `view_camera` applies `shakes[i]` before `fit_fov_y`.
- Rain: `Weather::frame_draws_sized(.., [cw, ch])`, the old `frame_draws` (`weather.rs` 753)
  passing `r.size()`; `rain_splashes` (`windowed.rs` 354-382) runs once per awake local camera with
  the splash budget divided among them, P1's call first. `Weather::splashes` reads the shadows'
  `in_view` flags set by the last `look()` (`weather.rs` 806-809, 924), so each view's `look()`
  (through `frame_draws_sized`) runs right before that view's splashes, never all looks first.
- Audio: section 8 with the scout's refinements: `Listener { pos, yaw, more: [Option<Vec3>; 3] }`,
  `ear(at)` the nearest listed ear, centre pan when more than one ear; every bare `ears: Vec3`
  parameter becomes `&Listener`. Music and voice follow the first awake local player
  (`WalkSim::first_awake`), as amendment A says. Foley culls against any view.
- LAN with local players: section 9 as packets B14-B20 (18.2). PROTOCOL 11 to 12. Local 0's
  `Inputs` chunk stays byte-identical and extra locals use a new `LocalInputs { idx }` chunk, so a
  one-local peer sends the same input datagrams. The handshake does change (PROTOCOL 12,
  `Hello.locals`), so the `lan_soak` pins are checked before B14 and re-pinned with a stated
  reason if admission moves them. LAN locals are seated by
  bundles: `WalkSim::adopt_local(slot)` (no `join_human`); RESPAWN goes in the submitted pad and
  `pads_with_local_respawns` is refused under `netplay.active`; `adopt_round` re-follows every
  local; `is_suspended` tells "not yet joined" from "away". Each machine records only its own
  locals' careers. Under `netplay.active` the lost and returning pad path only sends `LocalAway`
  / `LocalBack`; it never calls `suspend_local_player`, `resume_local_player` or
  `install_local_profile` directly (they change hashed sim state on one machine). Profiles enter
  the sim only through Hello, PlayerJoined and bundles.
- Game Menu settings rows edit the opener's own profile: `row_scope(Row) -> Player | Host`. Player:
  mouse sensitivity, invert, pad look, flight controls, rumble, rumble on fire, input icons,
  viewpoint, assist, career awards, FOV, the controls editor. Host: difficulty, AI cap,
  reinforcements, volumes, graphics, window mode, counters, cheats, the Splitscreen page.
  `persist_setting` (`menu_actions.rs` 6) splits into `persist_player` / `persist_host`. Effects
  per slot for an opener above 0: assist and pad look through `set_slot_assist`, career awards
  through `set_slot_career_award_bonuses`; `fov_scale` joins `LocalView` (swapped by `swap_local`,
  `walk/locals.rs` 224). Host rows show "Shared" in their help when the opener is not P1.
- Per-seat input settings in battle: bindings, gamepad layouts, flight controls, pad look,
  viewpoint, input icons and rumble come from that seat's profile (`split.rs` 200, 229-232,
  420-441, 548-575); `kbm.follow` uses the keyboard seat's profile.
- Round timer and banner per view: `ui::hud::draw_hud_in(ctx, m, rect, salt, scale)` (`hud.rs`
  1125); salt 0 keeps the bare Ids; in split, one per awake view under that cell's team counts.
- Polish (old phase 8): vehicles, flyers, space maps, class pickers and the death flow per view, a
  guided missile or droid not released by a suspend, an open pause menu on a suspend.

Supersedes: section 7's "no per-view egui overlay" (tags, timer and scoreboard are drawn per cell
on the one egui pass), section 7's "Controller assignment" pause row (Players screen only), and
section 17's band rule (`ui_rect` is kept for projection only).

Standing rules for every packet: the keyboard and mouse are never forced onto a player; seats
never renumber or swap on their own; a lost controller affects only its own cell; a double-click
launch with no flags; RotE and GC greyed; the one-player path stays bit-identical
(`scripts/one_player_gate.py`). New per-local code runs only when `local_count() > 1`.

### 18.2 Build order

One builder per packet. Tests are filtered to what the packet touches. "Gate" means
`scripts/one_player_gate.py` unchanged on every row. The gate covers the simulation only (hashes
at `--bench-frames 0` plus the LAN soak): it renders no HUD, saves no profile and plays no
audio. So packets that touch those also carry a named one-player check: "Save identity" is a
scripted one-player round whose saved profile entry (counters, class kills, medals, settings) is
byte-equal before and after the change; "HUD identity" is the `hud_draw::` vertex identity at
the gate's sizes and at 1280x1024; audio keeps the existing `Handed` pins. "Captures" means headless
`game/tests/split_capture.rs` PNGs of 2 side by side, 2 top and bottom and the 2x2 grid for the
maintainer to look at, not an assertion. The maintainer gets one playtest build after B10 and one
after B13, per the one-final-build rule; LAN gets its own after B20.

Visible items first:

- **B1 Profile core** (game `profile*`, no UI). `Profile::id`, `Store::bind`, `create_quiet`, the
  no-save rule for slot-less seat profiles, `SplitscreenSettings` with `normalize`, the per-player /
  host settings list as a documented const. Check: `profile::` tests: `bind` and `create_quiet` keep
  `last`; a slot-less seat profile never writes the `$OPENBF_PROFILE` file; the new field
  round-trips and an old file loads with defaults (`name_tags` true, identity `cells`); a
  partly missing `splitscreen` entry loads; a bad `cells` normalises to the identity; a profile
  with an unknown `two_player` value still loads, and so do the other profiles in the file.
- **B2 Players screen per-seat pickers** (ui `shell_ps2/players.rs`, `profiles.rs`; game
  `front/players.rs`, `local_seats.rs`, `front.rs`). Per-seat `SeatProfile`, per-pad menu input with
  repeat, the region layout, taken-profile greying, the Back rule, Continue only when all `Ready`,
  hand-off to the battle by `take` (no clone), `apply_host_profile` split out of the title
  picker's success block, the host rule, the press mapping (Back = East only), the title
  picker's greying. Check: `local_seats::` and `shell_ps2::` tests: a pad joining opens its
  own picker; B on List unseats only the presser; B on seat 0's List is refused while others are
  seated; a taken id is refused, and `App.profile`'s id is refused while seat 0 is `Picking`;
  Continue is refused while any seat is `Picking` (Start, Enter and South alike); a pad's input
  never moves another seat's picker; North no longer means Back; the keyboard seat alone types;
  P1 re-picking stays on the Players screen with the other seats and the session options as
  they were; an unbound Esc with pad seats seated changes nothing; a seated pad dropping changes
  only its region, and reclaiming it restores the seat and its pick; the title picker refuses
  Select and Delete on an id another seat holds. Plus a headless render of the 2- and 4-region
  screen.
- **B3 Battle hookup for seat profiles** (game `battle.rs`, `split.rs`, `walk.rs`,
  `walk/locals.rs`, `events.rs`, `career.rs`, `windowed.rs`, `close.rs`, `netplay/apply.rs`).
  `install_local_profile(walk, slot, &PlayerProfile)` split out of `netplay::install_profile` minus
  cheats, called only at first seating (after `add_local_player` in battle setup), never after
  `resume_local_player`: that already gives back the round's career, assist and cheats through
  `restore_slot` (`walk/locals.rs` 203-217, `awards.rs` 224-236), and `install_slot_medal_career`
  (188-208) would reset progress, earned bits and activations and clear `netplay.baseline`. On a
  resume only settings-only fields may be re-applied from the seat profile (the award-bonus
  preference), never the career or the baseline. After each extra career is installed,
  `split_feeds[k-1].set_opening_medals` under `with_local(k)` (18.1 item 3). The offline
  extra-slot gate and the medal-point loop (the extra feeds' tick points into
  `score_medal_points(local_unit(k), ..)`); `pending_class_kills` into `LocalView`; per-seat
  career flush and `record_round` at all four call sites (`events.rs` 1321 and 1623,
  `windowed.rs` 2373, `close.rs` 416), each routed by `seat_number(i)` into `App.profile` (seat
  0) or `App.split_profiles[seat]`, the one live copy (18.1 item 1). In `leave_battle` the
  per-seat records run before `self.split.take()` (`windowed.rs` 2336), or from a local-to-seat
  map snapshotted before it, and the profiles are moved back to `FrontShell` after. Per-seat
  input settings (bindings, flight, pad look, viewpoint, icons, rumble, `kbm.follow`).
  Check: `walk::locals` tests: P2's kills, class kills and medals land in P2's profile and P1's
  stay untouched; P2 pays its own class costs; with seats 0, 1 and 2 Ready, each local's kills
  land in its own seat's profile; two rounds in a row with P2, P2's counters add up across both;
  P2 earns a medal, the pad drops and comes back, and the medal, progress and active bonuses are
  still there (and `a_lost_controller_leaves_a_bot_and_the_others_play_on` still passes). Gate.
  Save identity.
- **B4 Names table and awards per view** (new `player_names.rs`; `split.rs`, `lan.rs`,
  `events.rs`, `tick_hook.rs`). The table and its writers and readers, `local_slot_of` in
  `battle_names`, per-view `medals_seen`, `note_medals` in the split loop (no hero mirroring,
  18.1 item 3). Check: `player_names::` tests (merge, default); `battle_names` uses
  `local_slot_of`; an `event_feed` test where P2 earns a medal: P2's feed shows the medal line,
  P1's shows "<P2> earned ..."; a kill, team-kill and death between P1 and P2 name both by
  profile name in both feeds; `scoreboard_model` for view 0 lists P2 by profile name, never
  "Player 2", and the same for view 1 and P1; the existing `event_feed::` and `round_stats::`
  name tests unchanged. Gate.
- **B5 Edge-anchored HUD layout** (ui only: `hud_draw.rs`, `hud_map.rs`, `hud_capture.rs`,
  `hud_squad.rs`, `hud_ambush.rs`, `hud_scope.rs`, `hud.rs`, `spawn_draw.rs`, `stats_draw.rs`).
  `Frame` (split cells only), the y-anchor rule and table, every call site and hit test. Can run
  beside B1-B4. Check: HUD identity: `hud_draw::` vertex identity at 640x480, 1280x720, 3840x1080
  and 1280x1024 for the single view (the narrow one-player window unchanged); in a 900x1600 cell health ends
  within 8 % of the bottom, the minimap starts within 8 % of the top, the reticle is centred in
  `proj`; hit tests round-trip in a tall cell.
- **B6 Full-cell paint** (game `split_ui.rs`, `hud_feed.rs`, `events.rs`, `windowed.rs`). Paint the
  whole cell, drop `letterbox`, `hud_aspect` (band) for the HUD models beside `view_aspect` (cell)
  for `fit_fov_y`, the keyboard player's own cell and own spawn page for the mouse (pointer step
  inside `with_local(keyboard_seat)`, `split_keyboard_first` gone), `split_capture.rs` 301
  updated (painted rect is the cell). Check: `split_capture` tests; a keyboard player at local 1
  clicks a post on their own spawn page; captures. Gate. HUD identity.
- **B7 Splitscreen options** (ui `menu.rs`, `menu/settings.rs`, `shell_ps2.rs`; game
  `split_layout.rs`, `split.rs`, `events.rs`, `front.rs`, `pause.rs`). `arranged`, the profile
  wiring, the Splitscreen page and rows (two-player split, P# screen, name tags, HUD size), the
  SplitModes row, the `ClosePage` fix. Check: `split_layout::` tests (identity equals `cells` for
  1-4; every permutation is a bijection onto equal cells; a move swaps exactly two; in 2P every
  arrow press on either P# row swaps the two cells on screen, whatever the absent seats'
  values); `menu::` tests
  (rows only with `split`, Back from Splitscreen returns to its Options row, the two-player row
  locked at 3-4); captures of top and bottom, swapped 2P, and 3P with the blank cell top-left.
  Gate.
- **B8 Per-view egui: name tags, round timer, banner** (game `events.rs`, `split.rs`, `lan.rs`,
  `name_tags.rs`; ui `hud.rs`). `view_camera` computed once per view per frame outside any blend
  into `view_cameras`, read by `build_view` and the tags, `Vec<NameTags>`,
  `draw_name_tags_in`, `draw_hud_in`, placement by `cell_of`. Check: a headless split capture
  check that P1's tag lands inside P2's cell and nowhere in P1's; `to_screen` with the cell aspect;
  `draw_hud` with salt 0 unchanged; LAN single view unchanged (the `lan` tag test, if none exists,
  one comparing the old and new draw calls).
- **B9 Scoreboard and stats per view** (game `events.rs`, `battle.rs`; ui `scoreboard.rs`).
  Per-seat `score_hold`, `draw_scoreboard_in`, `split_stats`, per-pad page cursors. Check: a
  `battle::` or `round_stats::` test that view 1's pages name P2 as "me" with P2's career; view
  1's "medals this match" counts from P2's slot and P2's opening counts; `stats_pages` for view 0
  list P2 by profile name and view 1's list P1 by profile name; captures at round end show each
  cell's own pages.
- **B10 Game Menu rows edit the opener's profile** (game `menu_actions.rs`, `events.rs`, `pause.rs`,
  `walk/locals.rs`; ui `menu.rs`). `row_scope`, opener-profile model values, `persist_player` /
  `persist_host`, slot-level assist and award effects, `fov_scale` in `LocalView`, the controls
  editor on the opener's profile. Check: `pause::` / `menu::` tests: P2 changing invert writes
  P2's profile only (the `App.split_profiles` entry that `record_round` also writes, so a
  round-end save after the change keeps it); P2's assist sets `slot_assist(1)` and leaves
  `sim.assist_intensity`; a host row from P2 writes P1's (seat 0's) profile. Gate. Save identity.
  Then the first playtest build.

Then audio, shake, rain:

- **B11 Multi-ear listener** (game `sounds*`, `event_sounds*`, `ordnance`, `ambience`,
  `announce.rs`). Check: filtered `sounds::` and `event_sounds::` tests: every pin unchanged; a
  two-ear listener hears a shot at ear 2 at ear 2's own gain, centred; a sound beyond both ears is
  culled; a looping voice re-gains when ear 2 walks up to it.
- **B12 Audio wiring** (game `windowed.rs` 60/133, `tick_hook.rs` 146, `events.rs` 1267/1370,
  `foley.rs`, music, voice). `listeners(walk)`, foley against any view, music and voice by
  `first_awake`. Check: on the `locals.rs` two-locals fixture, P2's shot is handed at near-full
  gain; one-local `Handed` rows equal before and after; music follows P2 while P1 is suspended.
  The maintainer listens (section 11 question 4 stays open).
- **B13 Camera shake and rain per view** (game `fx.rs`, `camera_shake.rs`, `weather.rs`,
  `windowed.rs`, `split.rs`). Check: `fx::` test: a blast beside P2 shakes `shakes[1]` and leaves
  P1's alone when far; `shakes[0]` sequence unchanged with one player; `weather::` test that
  `frame_draws` equals `frame_draws_sized(r.size())`; with two views, view 1's splashes read view
  1's `in_view` flags (each view's `look()` right before its splashes). Gate. Then the second playtest build.

Then LAN with local players (old phases 6 and 7):

- **B14 Admission** (net only). PROTOCOL 12; `Hello.locals`, `Welcome.extra`, `pick_slots`
  atomic with derived tokens, per-slot `PlayerJoined` / `PlayerLeft`, `Ready` per slot. Check: net
  tests: a 2-local join gets 2 slots; `Full` with 1 free; rejoin reclaims the same 2; a drop leaves
  2; a 1-local `Hello` round-trips. Before landing, run the `lan_soak` row: if PROTOCOL 12 /
  `Hello.locals` moves its pins, re-pin with the reason written in the gate script.
  B14's review added: each extra local's `LocalHello` names its stable seat (1-3) and its token
  is `local_token(token, seat)`, so a rejoin with a different line-up reclaims each seat's own
  slot; a reclaimed slot keeps its name only while its player asks for that name; a machine
  counts for hash agreement and the fell-behind drop while any of its slots is seated; a machine
  seated in part gets `PeerCaughtUp` for its unseated slots once the round runs.
- **B13b The maintainer's November 25 asks** (game only; independent of B15-B20, lands before
  B21 and before the next playtest build). (1) Name tags and the per-cell round timer use the
  game's HUD/menu font (the one the single-player HUD and menus draw with), not egui's plain
  font, scaled per cell as the rest of the cell's HUD is. (2) The split Game Menu says "Paused by
  <player name>" with the owner's profile name (the seat's `App.names` entry) instead of "Paused
  by P#", and the passed-on owner's name when the menu changes hands. Check: split captures in 2,
  3 and 4 cells showing tags and the timer in the HUD font (looked at, not only written); a menu
  test where P2 opens the Game Menu and the title reads "Paused by <P2's profile name>", and
  again after the menu passes to P1.
- **B15 Inputs** (net). `Chunk::LocalInputs { idx }`, `submit_inputs(&[PadState])` under one
  sequence, host routing, `Status.through` the min over the connection's slots. Check: linksim
  loss 0.03, both slots consumed at the same sequences; one-local client input datagram bytes
  unchanged (the handshake changed in B14; its soak re-pin, if any, is stated there).
- **B16 Host's own locals** (net). `Host::add_local`, `submit_input_for`, `host_local`. Check: host
  plus 1 local plus a guest gives the guest slot 2 and bundles carry slot 1's pads.
- **B17 Local away and back** (net). `LocalAway` / `LocalBack`, `set_local_away`. Check: in
  `virtual_net` a guest's local 2 goes and returns, only that slot changes, hashes match; the
  host's slot-0 `RemoveHuman` tested. B17 (with B20 for the game side) also makes the split.rs
  lost-pad path send only `LocalAway` / `LocalBack` under `netplay.active`: a `virtual_net` test
  where a guest's local 2 unplugs and replugs with no `suspend_local_player`,
  `resume_local_player` or `install_*` call, and hashes match. `LocalBack` reseats through
  `AddHuman`, whose `SlotQueue::seat_beside` (B15) lines the slot up with an active sibling:
  test that after local 2 comes back `consumed(local 1) == consumed(local 2)` on every tick.
  With local 0 away, the host's status takes pacing from the first active slot (B15): under
  linksim, local 1 shows no growth in starvations.
- **B18 Game netplay for several locals** (game `netplay/peer.rs`, `view.rs`, `predict.rs`,
  `walk.rs`, `walk/locals.rs`). Per-local pending and pacer runs under one clock, predictor over
  every local slot, `adopt_local`, RESPAWN in the pad, `adopt_round` re-follow, `is_suspended`
  split. Check: game `virtual_net` host 1 + guest 2 locals, 600 ticks, hashes match and both guest
  locals spawn and move; a predict test of both locals. Gate including `lan_soak`. It relies on
  every local of a machine consuming the same sequence (B15 grouped close and `seat_beside`); a
  test that the per-local pending queues stay equal after a late seat.
- **B19 Lobby** (ui `lobby.rs`; game `windowed/lobby.rs`). Rows grouped by machine, ready and kick
  per machine, `slot != 0` checks become `host_local` checks, the SplitModes Multiplayer row
  enabled, seats and seat profiles into `Hello` / `add_local`. Check: ui lobby test for grouped
  rows and kick by machine.
  From B18's review: the desync report names the machine's slot set
  (`(0..local_count()).filter_map(local_slot_of)`), not only `local_slot`; a local dropped through
  `Peer::set_extra_slots` (kick, a host local leaving) is released from the walk too
  (`WalkSim::release_local(slot)`, then a debug check that the walk's local slots equal the peer's).
- **B20 LAN window** (game `split.rs` 165, `events.rs` LAN branch). Split allowed in a LAN round
  through `adopt_local`; per-local pads into `n.view.frame`; split feeds and rumble on the LAN
  branch; names from the session into `App.names`; per-view name tags on the LAN branch (each
  cell tags remote players and the co-local, under the Name tags row); the lost-pad path as in
  B17; a further local's Game Menu Respawn taken with `WalkSim::take_respawn_of(i)` into its pad
  (never applied by the walk under `netplay.active`). Check: `--net-soak --locals 2` clean while the pinned 1-local soak is unchanged; a split
  capture of a LAN round with 2 locals shows a remote player's tag in each cell that sees him and
  the co-local's tag in the other cell. Gate. Then the maintainer's run: 2 + 1 players finish a round
  with matching hashes.
  B17's game side, each its own check (split stays refused in a LAN round until these land, so
  the offline suspend path never runs under `netplay.active`):
  - `split.rs` lost-pad path under `netplay.active` calls only `set_local_away` (host's
    `HostCommand::SetLocalAway`, guest's `ClientCommand::SetLocalAway`). Check: a game
    `virtual_net` test where a guest's local 2 unplugs and replugs with no
    `suspend_local_player`, `resume_local_player` or `install_*` call, and hashes match.
  - The returning local keeps its team and its round career (medals, award progress) the same
    way on every machine: `LocalBack` reseats on the slot's old team, not `TEAM_AUTO` (a tie in
    humans per team sends it to team 0), and the pad-loss removal keeps the career
    deterministically (a suspend/resume pair of session commands through `Sim::keep_slot` /
    `restore_slot`, no profile reinstall). Check: in that test the guest's local comes back on
    the same team with the same medal progress, hashes matching.
  - Any new link (a rejoin, or a new `ClientCore`) replays `set_local_away` for every local
    whose pad is still missing, since the host starts a rejoined slot not away. Check: rejoin
    with local 2's pad unplugged and the slot stays out.

Then polish:

- **B21 Polish** (old phase 8), a fixed list, one builder for the packet, each item with its
  named test. Gate.
  - Vehicle and flyer cameras per view: on the two-locals fixture P2 boards a speeder and then a
    flyer, and `view_cameras[1]` follows the vehicle camera while P1's is unchanged.
  - Space map HUD per view: a split space round builds a HUD model for each view, and no medal
    career is installed for any local (`awards.rs` 196-198).
  - Class picker per view: P2 opens the class picker on their spawn page and moves it with their
    own pad; P1's page does not move.
  - Death cam per view: P2 dies and view 1 shows the death camera and then P2's spawn screens,
    while view 0 keeps playing.
  - Suspend releases P2's guided missile or remote droid: the pad drops while guiding, and the
    missile or droid is released and `cam_guided` cleared for that local only.
  - Suspend closes the opener's pause menu: P2 opens the Game Menu, P2's pad drops, and the
    menu closes and the game resumes for the others (or passes to the next seated player, as
    today's rule says).

Order of dependence: B1 before B2 and B3; B2 before B3; B3 before B4's award lines (B4's table can
land first); B4 before B8 and B9 (both read `App.names`); B5 before B6; B6 before B7 and B8; B7
before B8 (the Name tags row); B3 before B9 and B10; B11 before B12; B14 to B18 in order, B16 and
B17 either way; B18 before B19 before B20; B13b at any time before B21. B3, B4, B8, B9 and B10 all edit `events.rs`, and B8
and B10 both edit ui `hud.rs` / `menu.rs` paths, so they land one after another, never in
parallel.

### 18.3 Deferred, and why

- Guest (unsaved) profiles (section 11 question 5): making one is a few button presses with the
  "Player N" suggestion, and an unsaved player loses their awards, which is what the request is
  for. Revisit if the maintainer asks.
- Per-player difficulty: out of scope (section 1); seat pickers skip the Difficulty page.
- Two seats on one profile: refused, not merged; merging would need a per-field career merge in
  `Slot::save`. The same clobbering between two processes on one profile already exists and stays.
- Loading the original's `2playerhud` / `4playerhud` trees: they cover top and bottom only and do
  not fit side by side or a reordered grid; the anchor rule covers all three.
- Headphone mode (section 11 question 4): after the B12 listening check.
- Mid-round joining of a new local player (section 1) and hotplug of a pad never seated (section
  4): unchanged.
- Off-screen marker clamping to the cell instead of the band: after the maintainer sees B6.
- Name length: profiles keep 10 characters; the net limit of 24 needs nothing.

### 18.4 Review of amendment B, November 24, 2026

A second Claude reviewed 18.1 and 18.2 against `d40cfdfbe`. Every finding was checked in the
code, and all of them hold except one detail, noted below. What changed:
- **Host and profile ownership (item 1, B2, B3).** Seat 0 is the host and always plays on
  `App.profile`, held as a `SeatProfile::Host` marker rather than a copy. Seat 0 cannot be
  unseated while others sit, so local 0 is always seat 0. Seat profiles 1-3 are moved into
  `App.split_profiles` with `take` and moved back after the battle. `record_round`, the flush
  and `persist_player` write that one copy, and every write is routed by seat number. The title
  picker refuses Select and Delete on ids that other seats hold.
- **Seat picker on P1 (item 1, B2).** It calls a new `apply_host_profile` helper. It no longer
  runs the whole `profile_action` block, which would go to `Page::Main` and reset `session` and
  `ia`.
- **Players screen (item 1, B2).** The press mapping is spelled out: Back is East only, and
  South and Start do not Continue while any seat is Picking. A seated pad that drops keeps its
  seat and pick, with a notice in its region. An unbound Esc does nothing while pad seats are
  seated.
- **Profile install (B3).** The profile is installed only at first seating, never after
  `resume_local_player`, so `restore_slot` keeps the round's medal progress. The extra feeds'
  opening medals are re-read after each install. Per-seat records run before
  `self.split.take()` in `leave_battle`.
- **Hero lines (item 3, B4).** The open "hero lines mirrored" question is decided: no mirroring.
  The finding's premise that `heroes::tick` writes lines was wrong, because it only ticks
  `self.feed` and scores medal points. The real gap is the dropped points of the extra feeds,
  and B3 adds them.
- **Name tests (B4, B9).** New checks that each view names the other player by profile name, on
  the scoreboard and on the stats pages, and that view 1's "medals this match" uses P2's slot.
- **Camera (item 2, B8).** `view_camera(walk, i, cell, look, shake)` does its own blend. It runs
  once per view per frame, outside any blend, into `view_cameras`, and `build_view` reads that.
- **Options (item 4, B1, B7).** `SplitscreenSettings` gets a hand-written `Default` and
  `#[serde(default)]` on the struct. The profile's own enums have a `#[serde(other)]` fallback,
  so an unknown value cannot make `profiles.ron` unreadable. With 2 players, the P# rows swap the
  two cells on every press. The keyboard pointer runs inside `with_local(k)`.
- **HUD (item 5, B5, B6).** `Frame` is used only for split cells, so a one-player window
  narrower than 4:3 stays identical; 1280x1024 is added to the vertex test. A separate
  `hud_aspect` keeps `view_aspect` for `fit_fov_y`.
- **Rain (item 6, B13).** Each view's `look()` runs right before that view's splashes.
- **LAN (item 6, B14, B15, B17, B20).** The "same datagrams" claim is narrowed to input
  datagrams, and the soak is re-pinned in B14 if the handshake moves it. Under netplay, a lost
  pad only sends `LocalAway` / `LocalBack`, with a `virtual_net` test. B20 gains per-view LAN
  tags, and the Name tags row rule is stated for LAN with 2+ locals.
- **Build order (18.2).** "Gate" is stated to cover the simulation only, and Save identity and
  HUD identity checks are added. B21 is now a fixed list of six items, each with a test. The
  dependencies gain B4 before B8 and B9, B7 before B8, and the shared-file serialisation.
- **Section 10.** Phase 2's "no feed lines until phase 8" is marked historical.

Duplicate findings (resume and install, the narrow-window `Frame`, and the three host and P1
ambiguity findings) were merged into one change each.

### 18.5 B1 result, November 24, 2026

Landed on `claude/split-b1` (game `profile*` only, no UI, no sim). `Profile::id()` (from the
private slot); `Store::bind(id)` (reads, attaches, never moves `last` and never writes the
catalog); `Store::create_quiet(name, difficulty)` (`create` without moving `last`, through a
shared `create_in` and `Catalog::insert_quiet`); a `#[serde(skip)] seat` marker set by `bind`,
`create_quiet` and `Profile::into_seat()`, so a slot-less seat profile never saves to
`$OPENBF_PROFILE` and `persists()` is false for it. New `profile/splitscreen.rs`:
`Settings.splitscreen: SplitscreenSettings { two_player, cells, name_tags, hud_size }` with
`#[serde(default)]` on field and struct, a hand-written `Default` (side by side, identity cells,
tags on, Full), `TwoPlayerSplit` and `HudSize` each with a `#[serde(other)] Unknown`,
`normalize()` (called from `Profile::normalize`, so every catalog and dev-file load),
`TwoPlayerSplit::layout()` to `split_layout::TwoPlayer`, `HudSize::k()` (1.0 / 0.85 / 0.7), and
`SETTING_SCOPES` / `setting_scope()`: every `Settings` field as `Player` or `Host` per 18.1 item 6.
Tests: `cargo test -p game --lib profile::`, 41 passed (6 new: `bind_and_create_quiet_keep_last`,
`splitscreen_settings_load_from_any_catalog`, `splitscreen_settings_round_trip`,
`a_slotless_seat_profile_never_writes_the_dev_file`, `splitscreen_cells_normalise_to_a_permutation`
and `every_setting_has_one_scope`, which checks the list against the serialized field names).
No gate run: B1 touches no sim, walk, input, net or per-frame path. Deviations: the plan's
"no-save rule for slot-less seat profiles" needed a marker the code did not have, so it is the
`seat` flag above; the two-player variant is spelled `TopAndBottom`; `settings_version` and
`recent_hosts` are listed (Player, Host) so the list covers every field. A saved profile now
carries a `splitscreen` entry, so B3's "Save identity" check compares against a build with B1.
Review fixes: `cells` and `name_tags` load through lenient `deserialize_with` readers (any RON
value; the wrong length, range or type falls back to identity / on), tested in the same catalog
test; an enum value carrying data (`two_player: Custom(2)`) still fails the file like any strict
`Settings` field, and the module doc now says so. Seat 0's re-pick goes through `select` / `create`
(moving `last`, unmarked); only seats 1-3 use `bind` / `create_quiet`. `bind` does not refuse an
id another seat holds, so B2 must test that both pickers grey out ids held by other seats, P1's
included. Rows with no `Settings` field get their scope in `row_scope` (cheats Host, the controls
editor Player).
Left: nothing for B1; B2 uses `bind` / `create_quiet` / `id` and B10 uses `SETTING_SCOPES`.

**B11 result, November 24, 2026.** Landed on `claude/split-b11` (game `sounds*`, `event_sounds*`,
`announce.rs`). `Listener` gains `more: [Option<Vec3>; 3]`, `Listener::new`, `with_more`, `ear(at)`
(the nearest listed ear, `pos` on a tie) and `dist(at)`. `side` returns 0 (centred) when there is more
than one ear. A small `Ears` trait (a bare `Vec3` or a `&Listener`) replaces every bare `ears: Vec3`
parameter of `SoundLib` (`start`, `tick`, `cry`/`cries`, `boost`, `weapon_voices`, `station_ambient`,
`hero_samples`, `ask_line`/`ask_line_with`, `ambient_heard`) and of `Commander::gate_calls`. The
plan said `&Listener` everywhere. The trait keeps the referees', examples' and tests' `Vec3` call
sites unchanged and gives the same arithmetic with one ear (`(pos - ear).length()`, same operand
order), so the one-player path is bit-identical. Loop levels, prepared takes and the mixer's distance
all use `listener.dist`. Ordnance hums rank and pitch each round from the ear nearest it, with the
velocity of the first `is_player` unit (the first ear's with one local player) and other ears taken
as still (OURS, `replace:` in B12: carry each ear's velocity in `Listener` rather than find it by
`is_player`). The windowed build's combat-call gating (`main/windowed/events.rs`) passes the whole
listener to `gate_calls`, as the headless SoundLog does, so the game and the log gate alike. The
SoundLog's logged distance is `Listener::dist` (nearest ear), bit-identical with one ear.
`event_sounds::each`, `each_off_handle` and `each_with` stay `ears: Vec3` (the first ear): their only
use is `Event::Command`'s order line, played at `ears`; B12 decides whether it plays at the issuing
local's ear. Nothing builds a multi-ear listener yet: `event_sounds::listener(walk)` still returns
one ear, and B12 wires the others. Tests: `cargo test -p game --lib -- sounds:: event_sounds::
announce:: blackbox::tests::sound` gave 76 passed, 0 failed, 4 ignored (audio-device and census
ones). That includes every existing pin unchanged and these new tests:
`a_one_ear_listener_measures_from_pos_to_the_bit`,
`a_second_ear_hears_a_shot_at_its_own_gain_centred`, `a_sound_beyond_every_ear_is_culled`
(one-shot, engine loop and unit line), `a_looping_voice_regains_when_the_second_ear_walks_up` and
`ordnance::a_second_ear_pitches_the_round_it_stands_by`. `cargo clippy -p game --all-targets` is
clean in the touched files. Integration rows: `--test suite -- hailfire hero_audio ambience_ps2 sbd`
gave 25 passed, 5 failed, and `--test space_playtest_fixes` gave 11 passed, 1 failed. Those same 6
fail identically on unmodified main at `26530897a`: missing Geonosis missions, a short hero sound
file, and no `imp_weap_ord_exp_lg` in the local bake. They are local bake gaps, not this change. The
one-player gate (release build of this branch) matches on every row, `lan_soak` included; it plays
no audio, so the one-ear arithmetic above is the audio identity. Left: B12 (`listeners(walk)`, foley against any view, music and voice by
`first_awake`) and the maintainer's listening check.

**B5 result, November 24, 2026 (branch `claude/split-b5`).** Landed in `ui` only:
`hud_draw::Frame` (`window(size)`, today's numbers, and `cell(size, hud_k)` for split cells:
`box_h = min(h, 0.75 w) * hud_k`, x by the thirds rule on the spare width, y by `VAnchor`, and
`proj`, the cell's 4:3 band for projected points in `split_layout::ui_rect`'s whole pixels), `draw_in`,
and `Panel` for the centred spawn pages, stats pages and end movie, with the footers at the real
bottom. The panel is `proj`'s band and does not shrink with `hud_k`, so the class page's boxes
stay on the 3D line-up and `class_at`'s `ui_rect` band at every HUD size. The y-anchor rule
and table are computed once in `Hud::new` (`Hud::v_anchors`, `Hud::v_anchor`). There are `_in`
twins of every hit test and point helper (`side_row_at_in`, `help_at_in`, `help_icon_point_in`,
`window_point_in`, `hud_map::{window_point_in, spawn_window_point_in, hit_spawn_post_in}`,
`hud_squad::{centre_in, label_anchor_in}`, `hud_capture::picture_in`,
`hud_ambush::visible_box_in`, `hud_scope::{quads_in, bars_in}`) and `root_y_span`. The old
window-size functions are kept as wrappers over `Frame::window`. Nothing in `game` calls the
new paths yet: B6 does that, and B6 also drops `letterbox`. The captures therefore still show
the letterboxed band. I looked at `two_side_by_side`, `two_top_bottom` and `four` at 1920x1080,
and they look as before.
Checks:
- `cargo test -p ui --lib -- frame_identity frame_tests hud_scope hud_capture hud_squad
  hud_ambush hud_map spawn_draw stats_draw hud_draw hud_tree`: 71 passed.
- HUD identity is `frame_identity::the_single_view_is_unchanged`. It hashes every vertex, box and
  code-drawn piece, plus every hit test, at 640x480, 1280x720, 3840x1080 and 1280x1024, the
  minimap's Small-view `window_point` on a layout with a Map root included. The pins were made by
  running the same file on `26530897a` before the change.
- `frame_tests::the_panel_is_the_ui_rect_band_at_every_hud_size`: at `hud_k` 0.85 and 0.7 the
  panel equals Full's and `ui_rect`'s band (900x1600, 683x1600, 960x1080, 1280x720), and the
  class page draws the same.
- `frame_tests` and `frame_identity::split_cell` cover a 900x1600 cell. In it, the bottom roots
  end within 8 % of the bottom, the top roots start within 8 % of the top, and the reticle
  matches a 900x675 window moved into the band. The side rows, help lines and spawn posts
  round-trip. The capture icon (top-right, under the minimap column), squad cross and chevrons
  go to their edges. A cell 4:3 or wider
  at Full equals the window.
- `HUD_ASSETS=<bake> cargo test -p game --test split_hud_edges -- --ignored`: 1 passed on the real
  1-player layout. `player1healthbar` ends at 94.4 % of the cell, and the minimap's origin is at
  7.2 %.
- `cargo test -p game --test split_capture -- --ignored`: 1 passed.

The one-player gate was not run, because B5 touches no sim, input, net or per-frame game path.
Deviations from the plan:
- `reticule` joins the Centre table, because by the box rule it would otherwise go to Top
  whenever it has no projected point.
- On the real layout, the rule puts `player1targethealthbar`, `player1weapon1lockon` and the
  teams' scores at the top, and `player1weapon2lockon`, `lockoninfo` and the tooltips at the
  bottom, as the plan intended.

Still to do: B6 wires `Frame::cell` into `split_ui.rs` and `hud_feed`, and the HUD size setting
in B7 feeds `hud_k`. The round-end banner sheet is centred on the cell and sized by the box, so
it shrinks with `hud_k`.

**B6 result, November 25, 2026 (branch `claude/split-b6`).** Each split view's UI is now laid out
for its whole cell and painted over it. `SplitUi::paint` calls the new `HudView::update_in` with
`game::split_ui::cell_frame(cell, hud_k)` (that is, `ui::hud_draw::Frame::cell`) and returns the
cell. `HudView::letterbox` and its `dy` are gone. The HUD models project through a new
`band_aspect`: the aspect of `ui_rect(cell)`, which is `Frame::cell`'s `proj`. `view_aspect` (the
cell) stays for `fit_fov_y` and the soldiers. The pointer belongs to the keyboard player
wherever they sit. `step_spawn_pointer_at` picks `(keyboard_seat, its cell)`. The new
`step_spawn_pointer_in` swaps that player's view into the walk with a new
`WalkSim::swap_local_view(i)` (the two halves of `with_local`). The window's step borrows more
than the walk, so the closure form does not fit. While swapped in, the step reads that player's
spawn stage, posts and spectate state. It hit-tests their own cell's frame with
`side_row_at_in`, `hit_spawn_post_in`, `class_at` on the `proj` band with their own preview
(`split_previews[k-1]`), and the new `HudView::spawn_help_at_in(k, ..)`, which reads that
player's HUD state whether it is selected or parked. A pointer outside the cell, or a player who
is away, does nothing. The `split_keyboard_first` gate is gone: the pointer steps whenever
someone holds the keyboard. The look preview at the old line 1340 now tests `keyboard_seat != 0`,
which behaves as before. In `split_capture.rs` the painted rect is the cell.

Review fixes: issue #128's mouse-look hand-back now follows the same player as the pointer. A new
`App::mouse_seat` (the keyboard seat when split, `None` with nobody on it, else 0) keys
`can_grab`, `on_spawn_screen` and so `regrab_after_spawn` and `note_pointer_spawn`; a suspended
player is never behind the screens. `step_spawn_pointer_view` now returns whether the pointer
accepted, and `note_pointer_spawn` runs after the swap back, so it reads the keyboard player's own
unit. Before, a keyboard player on a later seat lost the click-free regrab, or got it at P1's
respawn. The spectate click (issue #182) now also counts only inside the keyboard player's cell
(the shared `in_cell`). New test
`a_keyboard_player_on_a_later_seat_gets_mouse_look_back_on_their_own_spawn` (keyboard on local 1,
P1 dead throughout: a keyboard spawn and a pointer accept each give the grab back on the spawn
frame). Bin filter `... windowed::split mouse::`: 28 passed; the one-player gate matches again.

Checks:
- `cargo test -p game --bin game -- spawn_pointer keyboard_player click_on spawn_help
  double_click`: 8 passed. That includes the new
  `a_keyboard_player_at_local_one_clicks_a_post_on_their_own_spawn_page`: the keyboard is on local
  1 (slot 1, the right cell of 1280x720 side by side). A click over post 2's place in P1's cell
  does nothing. Hovering post 2 in their own cell picks it for slot 1, and the click accepts it,
  so slot 1 moves on to Class. Slot 0's stage and post are unchanged, and the walk is handed back
  to P1. Removing the swap makes this test fail.
- `-- windowed::split`: 7 passed.
- `cargo test -p game --lib -- split_ui split_layout hud_feed walk::locals`: 20 passed, 7 ignored
  (asset-bound).
- HUD identity: `cargo test -p ui --lib -- frame_identity`: 4 passed, with
  `the_single_view_is_unchanged` among them. No ui file changed.
- `cargo test -p game --test split_capture -- --ignored --test-threads=1`: 1 passed. It needed
  the shared `assets_baked` and `missions` symlinked into the worktree. I looked at
  `two_side_by_side`, `two_top_bottom`, `two_class_side_by_side`, `two_round_end` and `four` at
  1920x1080. In the 960x1080 cells, the scores, target, feed and minimap sit at the top edge,
  ammo, health and stamina at the bottom edge, and the reticle in the middle. The spawn and class
  pages are centred with their help lines at the cell's real bottom, and the veil and banner cover
  the whole cell. The 4:3-or-wider cells (2x2, top and bottom) look as before.
- The one-player gate (release build of this branch): every row matches, `lan_soak` included.
- `cargo clippy -p game --all-targets`: no new warnings in the touched code.

Deviations from the plan:
- `SplitUi` gained `hud_k`. The window already passes the host profile's
  `splitscreen.hud_size.k()`, and the pointer reads the same value. There is no menu row yet, so
  in practice it is always 1.0. B7 adds the row.
- The pointer swap is a two-call `swap_local_view`, not a `with_local` closure, for the
  borrowing reason above.
- `ui_rect`'s doc now describes the projection and line-up band. It no longer describes the UI
  rectangle.

Left: B7 wires `arranged` and the Splitscreen page, including the HUD size row that feeds
`hud_k`. Off-screen marker clamping to the cell (18.3) waits for the maintainer's look at these
captures.

### 18.6 B2 result, November 24, 2026

Landed on `claude/split-b2` (ui `shell_ps2/players.rs`, `profiles.rs`, `spawn_draw.rs`; game
`local_seats.rs`, `main/windowed/front*.rs`, `windowed.rs`; input `owners.rs`). The Players
screen gives every seat its own profile: `SeatProfile = Open | Picking(Picker) | Host |
Ready(Profile) | Lent(Option<u64>)`, kept per seat number in `game::local_seats::Party` with the
seats. `Party::step` takes each pad's own presses (A, Start, B, X, Y, the arms with a per-pad hold
repeat and the left stick, L1 / R1) and the keyboard seat's keys, typing and pointer, and returns
the step, the cues and seat 0's new profile. P1 is the host (`App.profile`); P2-P4 open their
picker on joining and are bound with `Store::bind` / `create_quiet` (host's difficulty, no
Difficulty page, no Delete); seat 0's pick goes through `select` / `create` and the new
`App::apply_host_profile` (split out of the title picker's success block), so the Players screen,
the other seats and the session options stay. Ids other seats hold (P1's included, also while
seat 0 chooses again, and lent ones) are greyed and refused in every seat picker; the title
picker greys P2-P4's and refuses Select and Delete on them (`local_seats::title_refuses`). Back
is East only: name keyboard to list, Ready card to picker, list to an empty seat (the presser's
only); seat 0's list refuses B while others sit; an unseated pad's B or an unbound Esc leaves
only an empty screen. Continue (Start, A on a Ready card, Enter on the keyboard seat's Ready
card, Space, a click) needs two or more seated and none choosing. A dropped pad keeps its seat
and pick (its region shows the notice); the same pad back restores both, another pad reclaiming
the seat reopens its picker (`PadOwners::lost_pad`; with no catalog it starts afresh, the host on
seat 0, an unsaved guest elsewhere). The screen is two regions (P1 top, P2 bottom) while one
seat at most is taken, else the 2x2 grid (so P3 and P4 show their join prompts), each a whole shell screen
(`Picker::draw_in` / `target_in` / `screen_in`, help on the region's box through
`help_icon_on`), prompts in each seat's own device's icons; a Ready card's B reads "Change profile" when it
reopens the picker, and A / Enter "Continue" once Continue is live. The keyboard seat's Enter
accepts its typed name (Space is a letter there), and the wheel scrolls its list. Every split
round that starts loading (`App::next_round`: the launch's first round, each next round, a
Restart Mission) moves P2-P4's Ready profiles into `App.split_profiles` (`Party::lend`, the
seat becomes `Lent`), and every return to the shell (`App::init_shell`: a left battle or a
load that failed) moves them back (`give_back`); nothing is cloned. Seat 0's re-pick also
clears the campaign save list's current save.
Review fixes added `local_seats::` tests (Enter accepts a typed name; another pad on a lost
seat with no catalog) and `front::profiles::tests::split_profiles_are_lent_every_round_and_back_on_the_shell`
(nothing lent when no row loads; lent at each of three rounds, back on its seat after each
leave and after a failed load); 28, 31 and 4 passed, the captures redone and looked at, the
gate again every row matches. Kept: a newcomer pad still takes a waiting seat before a free
one (`PadOwners::reclaim`), so a pad that re-pairs under a new id returns to its player.
Tests: `cargo test -p game --lib local_seats::` 27 passed (12 new: own picker on join, B
unseats only the presser, seat 0's B refused while others sit, taken and host ids refused,
Continue refused while choosing by Start / Enter / Space / click / A, a pad moves only its own
picker and North is not Back, only the keyboard seat types and points, seat 0's pick changes
only the host, unbound Esc changes nothing, a dropped pad and its return or another pad's
reclaim, the title picker's refusals, lend and give back; the old Party tests rewritten to the
new rules); `cargo test -p ui --lib shell_ps2::` 31 passed (the Players test now checks the 2
and 4 regions, the picker's hit in its region and Continue); `cargo test -p game --bin game
front::profiles::tests` 3 passed (1 new: seat 0's pick keeps the Players page and the session).
Headless render: `UI_CAPTURE_OUT=engine/artifacts/split_captures/players cargo test -p game
--test ui_capture capture_players -- --ignored` wrote `players_{2,4}_{4x3,16x9,21x9}.png`
(looked at: two and four regions, picker, name keyboard, lost notice, open seat). Gate after
`cargo build --release -p game`: every row matches, `lan_soak` ok.
Deviations: the seat profiles live in `Party` (on `FrontShell.party`), not a separate
`FrontShell.seat_profiles`, so the rule tests without a window; `Lent` holds `Option<u64>`; with
no profile catalog (a `$OPENBF_PROFILE` dev file) P2-P4 sit at once on unsaved "Player N"
profiles and B unseats them; B on the host card alone empties seat 0 at once (as before), with
others seated it opens seat 0's picker; on the seat name keyboard North is the symbols page and
West Backspace (the title picker's East is Back there); the keyboard seat re-picks with Esc (no
West key); regions follow seat numbers (four once P3 or P4 sits), and every Ready card carries a
Continue bar any click may press; the title picker's refusal is tested through
`title_refuses` rather than a window test, since a pad seat needs a real gilrs id.
Left: B3 reads `App.split_profiles` (records, careers, per-seat input settings); the maintainer
looks at the captures and the screen with real pads.
