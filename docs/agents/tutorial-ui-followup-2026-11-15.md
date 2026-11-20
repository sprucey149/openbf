# Tutorial UI follow-up — 2026-11-15

Scope authorized by the maintainer's ten comparison images: remove the tutorial's false command-post beam and capture meter, resolve localized control tokens for the device and current bindings, restore repair-target/world/map markers, and draw the holocron. Continued the existing `feature/rise-of-the-empire` branch and staged campaign implementation; no unrelated staged changes were reset.

Source base: `03196218` plus the existing staged campaign work and this working-tree follow-up. The latest local black box (`engine/artifacts/playtest/last.json`) is an exit snapshot of `bf2_geo1_con.ron`, 68 ticks, built from `a3787c89e221+dirty`. It is an older Conquest run, not evidence of the reported campaign stages. The original-game evidence for this change is the maintainer's attached comparison frames 2, 5, 7, 8 and 10; no new original-game trace or slot-8 capture was taken.

## Changes

- Classify beam-free posts from every merged scene instance, including objective-only control zones absent from the Conquest layout. Existing hologram and post-light submission both use this classification.
- The right-side ring/spokes and small pointer are the capture meter (`ui::hud_capture`, `hud_flag_timer` and `hud_map_northpointer`). Skip uncapturable posts when choosing that meter; capturable posts retain it.
- Resolve tutorial control aliases through the active device family and gameplay context's controller/keyboard bindings. Use the same token renderer for text elements and event-message lines, including `{Map}`. Ordinary text keeps its existing shadow/alignment path.
- Preserve explicit entity/class markers on destroyed repairable stations. Emit campaign markers to the minimap, including map-only region markers and markers with world hints disabled. Respect team filtering and script removal.
- Load script-created flag models from their class's GeometryName and CarriedGeometryName, using map/common assets. Draw the holocron from its live campaign flag state. Exclude generic CTF overlays/models from campaign presentation.

## Verification

- `cargo check --manifest-path engine/Cargo.toml -p game --all-targets`: passed.
- `cargo test --manifest-path engine/Cargo.toml -p ui hud_draw::text::tests`: 3 passed, including all six device families and a remapped grenade prompt.
- `cargo test --manifest-path engine/Cargo.toml -p game --lib campaign_runtime::tests -- --nocapture`: 6 passed. Includes 20 seconds of startup for every listed campaign mission and the tutorial's first objectives. This is startup coverage, not full mission acceptance.
- `scripts/gpu_tests.sh --game-lib --include-ignored tutorial`: 5 passed. Headless, serial; covers repair/map-only/team markers, the false capture meter, beam suppression, three device prompt renders, and actual holocron drawing/removal.
- `cargo build --release --manifest-path engine/Cargo.toml -p game --bin game`: passed; rebuilt `engine/target/release/game.exe`.
- `git diff --check`: passed.

Local captures (ignored, contain owned game art): `engine/artifacts/tutorial-ui/prompts-{ps2,xbox,keyboard}.png` and `holocron.png`. Prompt captures and the visible holocron capture were visually inspected. No game art or localized strings were added to Git.

The size report was executed through its embedded Python because Windows resolves python3 to the Store launcher. With POSIX-normalized relative paths, its map check still reports seven existing unmapped source files, four named paths missing, and stale counts elsewhere; see the ignored `engine/artifacts/tutorial-ui/size-report.txt`. Entries for this follow-up and the continued campaign modules were updated. The branch already has source files over the size target.

## Remaining acceptance

Replay the complete tutorial in the windowed game, especially the repair stage, minimap objective ring and holocron pickup/carry/drop. Verify live keyboard/controller switching and custom control layouts during play. Slot 8 remains available for a fresh original-game comparison. The captures verify rendering paths, not pixel parity or a full original-game playthrough.

Source patch SHA-256 (`git diff HEAD -- engine/crates`): `a485897d01868de97aec1fee9b4ae46e21e94e1fb2933830db3f4b0176abeafc`.


## Second playtest follow-up

Authorized scope: the maintainer's next four comparison frames and five findings (holocron halo, cliff-post class interaction, full map/objectives screen, camera input during objective popups, and the scripted Mace transition).

Current evidence: `dist/openbf_data/playtest/last.json`, schema 18, exit at tick 17664 / 588.8 seconds of `bf2_geo1c_c.ron`, build `031962180841+dirty`. This is the packaged campaign playtest; the earlier `engine/artifacts/playtest/last.json` belongs to a different run. The original screenshot supplies the map/objective layout and holocron halo. `analysis/re-point-lights.md` sections 4 and 10 document the flag's LightOdf and native lens flare; its referenced native helpers are FUN_00468238 / FUN_00468a28. The implementation reuses the existing flare fans/ring and reads the owned class's color, intensity and draw distance.

Changes:

- Script-created flags submit their LightOdf flare at the live loose/carried model pose. Captures: `engine/artifacts/tutorial-ui/holocron-glow.png`.
- Campaign posts load their owned SwitchClassRadius, as Hunt posts already did. The interaction scan accepts an owned post in that radius when its capture region has been removed by the script, and excludes killed posts. Conquest's region-based rule is unchanged.
- Tab / Select opens the campaign map and objective browser; Up/Down selects an objective, Enter or Use opens its details, Reload or the menu's Square opens a hint, and Tab / Select or Back returns. The whole-map view retains units, player heading and scripted markers. Objective text, details, hints and footer strings are read from local data.
- Popup/browser frames discard pending mouse preview and use a neutral controller preview. Simulation and world effects hold; HUD fades and tutorial narration can continue.
- The campaign roster loads its scripted hero kit even if Instant Action heroes are off. Automatic score unlock is suppressed for campaigns; SetHeroClass, UnlockHeroForTeam and AcceptHero drive the existing hero replacement/spawn flow without a death or ticket loss.

Validation so far: 10 serial headless tutorial checks passed through `scripts/gpu_tests.sh --game-lib --include-ignored tutorial`; 18 core class-change checks passed; 37 core hero checks passed; game binary check passed. Map captures for PS2 and keyboard prompts and the halo capture were visually inspected. Final browser-layout capture and release/dist rebuild are recorded after completion below.

Remaining acceptance: play through the tutorial again with both input devices; compare the flare against slot 8 in the original game, and verify the class switch and Mace spawn at their actual scripted stages. The tests cover the underlying gates and rendering paths, not a full windowed tutorial run. No game data is tracked.


Final second-follow-up checks: the browser layout capture passed after uppercase text and bounded scrolling; the release game build passed. Updated `dist/openbf.exe` directly from that release binary because the existing data archive already contains the unchanged assets/missions needed by these code changes. Verified executable SHA-256 equality (`1eeaba0404711a8ae815af096dcc132d3915495681446ebdb8213e67328ab449`) and the existing archive footer. The archive and profile data were retained. `git diff --check` passed. The PS2 and keyboard browser captures were inspected again after the final layout change.

Second-follow-up source identity: `03196218+dirty`; SHA-256 of `git diff HEAD -- engine/crates` followed by the new campaign_map.rs bytes: `11da3a9e1c2b11829cfbcb5b9bd4d028a05e8d04236fd09ec8939fb53507aeb2`.
