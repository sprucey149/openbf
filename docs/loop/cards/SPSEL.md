---
id: SPSEL
title: The player's spawn POST lives in the sim: the PS2's spawn-map pick (a push takes the own-team post with the lowest along + 0.25 x perp^2, then keep-or-nearest), the respawn at THAT post, and the class row's names from the disc's text - the class choice is already in core (Sim::spawn_choice); the post cursor is still UI-only and the sim picks the node itself
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Evidence
analysis/PLAN_LOG_2026.md §ARCHIVED 2026-09-25 HANDOFF-OPUS.md §Standing backlog 3. PLAN §LOCKED FEATURE SET (classes per side).

## Notes
- UAUD1 (2026-09-29): the PS2's class-select names and equipment lists per class are in
  `engine/artifacts/lead_tmp/unit-audit/pcsx2/{cis,rep}_classes_sheet.png`, `end1_left_sheet.png`,
  `end1_imp_sheet.png` (e.g. WOOKIEE SMUGGLER: Bowcaster, Grenade Launcher, Time Bomb). The port's `label` rule gives
  "Repair" / "Trooper" / "assault" for these; the localised names are not baked.
- UAUD2 (2026-09-29, analysis/unit-audit-bf2.md §3.1, §3.8): BF2's class select (after a post on the spawn map): Left / Right cycle the
  classes (Down does nothing); each entry shows "Unit Count: N" (live on the team) and, for a capped class, "Max: 4",
  then "Equipment:" = ch0 then ch1 in slot order, award slots hidden. Names: Clone Trooper, Heavy Trooper, Clone
  Engineer, Clone Sharpshooter, Clone Commander, Jet Trooper; Super Battle Droid, Assault Droid, Engineer Droid,
  Assassin Droid, MagnaGuard, Droideka; Rebel Soldier, Rebel Vanguard, Rebel Smuggler, Rebel Marksman, Bothan Spy,
  Wookiee Warrior; Snowtrooper (Hoth), Shock Trooper, Imperial Engineer, Scout Trooper, Imperial Officer, Dark
  Trooper. Locked classes are black silhouettes with "Get N more points to unlock!" (-> UNLK).
  Frames: `lead_tmp/unit-audit/pcsx2_bf2/*_sheet.png`.
- CWSP RE (2026-10-02, `analysis/re-command-walker-post.md` section 5, verifier-checked + a live BF2 session): the
  PS2's spawn-map pick, both games (B2 `FUN_0039a058`, B1 `FUN_002e03b0`): from the current post, a stick / D-pad push
  takes the scheduled post of the player's own team with a positive projection on the push direction and the lowest
  `along + 0.25 x perp^2` (positions = the owners' live ones); the result goes through keep-or-nearest and becomes
  the member's post. A human keeps a post while its owner lives and the team may spawn there, else the nearest
  eligible one. A command walker's post (AT-AT, AT-TE) is in the list like any other: live, one `Right` on BF2 Hoth's
  spawn map put the ring on an AT-AT and the player spawned beside it (`lead_tmp/cwsp/ps2/c2_spawnmap_sheet.png`).
  BF1 names the selection by the post's Label ("AT-AT"). Card CWSP adds the walker's entry to `spawn_model`.

## Planner re-scope (2026-10-02 09:30) - what is left, with files and a runnable acceptance
The old title ("the spawn screen still spawns ONE class per faction") is no longer true. Read in the tree at d3242d7:
`core/src/sim/spawn.rs` `spawn_select` moves `Sim::spawn_choice[slot]` with `buttons::SPAWN_CLASS_NEXT` / `PREV`, it is
hashed (`the_spawn_select_state_is_part_of_the_hash`) and `respawn_slot` builds the respawn from it. What is NOT in the
sim is the POST: `ui/src/spawn.rs`'s header says the post cursor "is still UI-side only: the sim picks the spawn node
itself", and `game/src/main/battle.rs` `spawn_model` passes a UI cursor. So the player cannot choose where to spawn.

**Do.**
1. The post choice in `core`: a per-player chosen post, moved by pad bits the input log carries, read by
   `respawn_slot` (a node of THAT post). The rule is the PS2's, both games (`analysis/re-command-walker-post.md`
   section 5; B2 `FUN_0039a058`, B1 `FUN_002e03b0`): a push takes the scheduled own-team post with a positive
   projection on the push direction and the lowest `along + 0.25 x perp^2`; then keep-or-nearest (a human keeps a
   post while its owner lives and the team may spawn there, else the nearest eligible one).
2. A walker's post (CWSP) is in the list like any other - hence `deps: CWSP` (it also shares `sim/spawn.rs` and
   `main/battle.rs`).
3. The class row's names from the disc's text: EVMSG baked `common_bf{1,2}/locl/english.ron` (`manifest::hud::HudLocl`);
   the names the PS2 shows are in the UAUD notes above ("Clone Sharpshooter", "Snowtrooper", ...).
Not here: the PS2 spawn screen's own look (the egui panel stays), unlocks (UNLK), SPST's AI post draw (it should reuse
this card's "a member's chosen post" state rather than add a second one - note left on SPST).

**Acceptance (run and paste).**
- Rule test (`core`): four posts on a plane; for each of the four push directions the picked post is the one the
  formula gives, and a push with no post ahead keeps the selection. With CWSP landed, BF2 Hoth's live sequence from
  the opening post (`lead_tmp/cwsp/README.md`, "live BF2 spawn map"): Right -> list index 7 (an AT-AT's post),
  Down -> 8, Down -> 4, with the posts' positions from that state.
- Respawn test: a dead player with post K chosen respawns on a spawn node of post K, for three different K; when K is
  captured before the respawn, at the nearest eligible post. Mutation: with the read of the choice removed the test
  fails (paste).
- Replay: the choice is in `Sim::hash` once it leaves its default (STTAP's lesson: hash new state only off its idle
  value, so the pinned `--sim-hash 100` holds); a recorded pad log with two post changes replays to the same hash.
- The idle sweep player: sim, simhash, sweep, arena identical (a player who presses nothing spawns as today). If
  they move, declare `sim` and say why.
- Names: a test over the six showcase missions that every fielded class resolves a name from the locl table (paste
  the list for bf2_hot1 and bf1_nab1); `goldens/tat1_spawn.png` declared, viewed.
- FEEL_NOTES: the pad and keyboard bindings for the post cursor.
