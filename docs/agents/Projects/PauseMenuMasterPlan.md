goal: 
analyze the state of the in game pause menu UI and progress towards this goal: Analyze OpenBF's current pause menu implementation and create a complete plan to recreate the original Star Wars Battlefront II (2005) pause menu experience as closely as possible.
Requirements:

- Preserve existing OpenBF functionality.
- Replicate the original SWBF2 pause menu layout as accurately as possible.
- Match:
- menu structure
- visual hierarchy
  - spacing
  - alignment
  - colors
  - fonts (or closest available equivalent)
  - selection indicators
  - transitions
  - navigation behavior
  - controller navigation
  - keyboard navigation
  - sound behavior
  - menu animation behavior
Cheat Menu Extension:

The recreated pause menu should also include an OpenBF-specific Cheats section that feels native to the original game.

Design a location for:
- Existing cheat options
- Future cheats

The cheat menu should appear as if it could plausibly have shipped in the original SWBF2 rather than feeling like a developer/debug menu.

Plan:

<br>
---
<br>
# Pause Menu Parity Analysis — OpenBF vs Original SWBF2 (2005)
<br>
## 1. Architecture analysis
<br>
**The framework is egui** (`engine/crates/ui`), immediate-mode, drawn over the frozen battle frame. The pause menu is pure state + draw: a `MenuModel` goes in, a `Vec<MenuAction>` comes out, and `game::pause::apply_menu_action` executes them against `WalkSim`.
<br>
Data flow (in-game pause):
1. winit pad/keyboard events → `App` (windowed) builds `ui::menu::MenuInput`
2. `ui::menu::navigate()` moves the cursor and produces actions; pointer clicks are produced inside `draw_menu()` itself
3. `game::pause::apply_menu_action()` mutates walk/session/settings; `cheats.follow(&walk.sim)` re-syncs flags
4. Sounds: `ui::shell_feel::MenuSound` names, played through the game's sound library (`front.cue(...)` in shell, `sl.play_2d(...)` in-game)
<br>
Key existing subsystems relevant to parity:
- **`ui::shell_feel`** — already a faithful port of the BF2 PS2 shell "feel": the 7 menu click sounds with exact sample names/lengths/pitch/gain, the `SELECT_INTERVAL` rate limit, the focus `Pulse` (0.3–1.0 alpha ping-pong at 2/s stepped at 30 Hz), `flashy` text entrance, `HoldRepeat`, `ShellPlaylist`. **This is reusable for the pause menu.** Today only the shell screens (`shell.rs`, `shell_ia.rs`, `shell_ps2.rs`) use it; the in-game pause menu plays Accept/Select ad-hoc.
- **`RowList`** (in `menu.rs`) — whole-row scrolling list with settle-on-row behavior. Close analog to the PS2 `listmanager`.
- **Morse cheat recognizer** (`ui::menu::MorseCheats`, driven by `game::cheats::Session`) — a complete, tested port of the PS2 D-pad Morse system per `analysis/re-cheat-codes.md`.
<br>
### Engine limitations for a "perfect" recreation
<br>
1. **No 9-slice texture skinning in the current pause draw.** The original draws `border_3_pieces` window frames with `bf2_buttons_topleft / title_center / topright` title bars, and the `listbox_cursor` focus box is also a 9-slice sprite. OpenBF's pause menu uses egui `Frame` fills (black alpha 200, rounded corners). egui *can* render textures as image widgets/uv-rects, so this is achievable, but requires loading `.buf` texture assets from the player's own game data and a custom painter pass.
2. **Fonts**: original uses `gamefont_tiny/small/large` (line heights 15/18/22) — proprietary bitmap fonts in the game data. OpenBF currently uses egui's default font at sizes 18/28. Closest-equivalent licensing-safe fonts must be chosen; exact metrics can be matched by measuring text advance widths.
3. **Dimmed backdrop**: original `bDimBackdrop` dims the 3D frame behind the menu. OpenBF pauses the sim and draws a translucent panel, but does not dim/desaturate the render output. Requires a post-draw tint on the battle render target (render layer change).
4. **Determinism constraint**: `core` is deterministic with fixed ticks (`TICK_HZ = 30`, `TICK_DT = 1/30`, accumulator capped at 0.1 s in `WalkSim::advance_with`). A Time Scale cheat cannot scale tick dt; it must change *how many ticks per real second* are stepped (the accumulator rate). This is feasible and keeps determinism, but animation/render blend (`render_blend`, `pulse = tick/30`) and sound pitch must all key off the same scale factor.
5. **No menu transition animations in the original** — confirmed: BF2 has no transition whoosh (`shell_transition1..10` exist, no screen uses `movieIntro`). So transitions are *not* a parity gap; the only animations to match are the focus pulse, `flashy` title entrance, and the random-start-delay info text.
<br>
## 2. File inventory
<br>
| File | Role | Lines |
|---|---|---|
| `engine/crates/ui/src/menu.rs` | Pause menu model, rows, navigation, draw (`draw_menu`, `row_label`, `RowList`) | 951 (+3 test files: `tests.rs`, `class_picker_tests.rs`, `pointer_navigation_tests.rs`, `morse_tests.rs`) |
| `engine/crates/game/src/pause.rs` | Action dispatch against `WalkSim` (respawn, class picker, resume) | ~200 |
| `engine/crates/game/src/cheats.rs` | Session cheat registry + Morse recognizer wiring | ~130 |
| `engine/crates/core/src/sim/state.rs` | `Cheats` flags (6 cheats), `entries()`, `id_for_word` | lines 55–125 |
| `engine/crates/game/src/main/windowed/events.rs` | In-game pause event loop: pad→MenuInput, click actions, accept sound, music slider live volume | — |
| `engine/crates/game/src/main/windowed/front.rs` | Shell-side pointer click → `cue(MenuSound::Accept)` | — |
| `engine/crates/ui/src/shell_feel.rs` | Menu sounds, Pulse, flashy, HoldRepeat, playlist (the reusable "feel" kit) | 526 |
| `engine/crates/ui/src/shell.rs`, `shell_ia.rs`, `shell_ps2.rs` | Shell screens — **already closest to original SWBF2 menus** (built from `analysis/re-bf2-menus.md` + menuref captures) | 911/1095/1054 |
| `engine/crates/ui/src/input_icons.rs` | Button-prompt icon faces | 568 |
| `analysis/re-bf2-menus.md` (1365 lines), `analysis/re-cheat-codes.md`, `analysis/re-bf2-shell.md` | The reverse-engineering evidence base: layout numbers, sounds, pause menu row table §ifs_pausemenu | — |
<br>
**Uncommitted work in tree (validated last session, 20 passed ui tests, clippy/fmt clean):** `step_slider_action()` shared slider stepping for pointer+pad, Assist row help, Accept sound on pointer clicks of sliders, live music volume follow while paused. This is behavior parity work toward the navigation objective and should land as its own PR first.
<br>
## 3. UI comparison — Current vs Original
<br>
Original pause menu facts (from `analysis/re-bf2-menus.md` §ifs_pausemenu, [V]):
- Title **"Game Menu"** (`game.pause.title`), stack centered at screen y 0.5, **width 260 units**, ySpacing 5, `bDimBackdrop`, `bAcceptIsSelect`.
- Rows (PS2 retail): **Resume Game · Options · Respawn · Restart Mission · Quit** (Player List net-only; Free Camera/Fake Console/Buddies hidden in retail).
- Cross = accept (open plays `shell_menu_accept`), Triangle = resume (`shell_menu_exit`), popups Yes=accept / No=cancel, moves = stack click.
- Options row opens the pause sub-stack: Game Options (391.7-wide window, 5 rows of 48, `gamefont_large`, in-game centered), Audio Options (5 sliders + Output Mode), Controls, Online Options.
- Focus box = `listbox_cursor` 9-slice over the row; title bar yellow `(246,235,20)` with `flashy`; info text blue `(138,160,244)` with random start delay 0–0.5 s.
- Sounds: focus move `shell_select_change` (63 ms, rate-limited 0.1 s), accept 428 ms, exit 320 ms, cancel/error.
<br>
Current OpenBF pause menu:
- Title **"PAUSED"** centered, black rounded panel, white bold text — not "Game Menu", no title bar skin, no yellow.
- Rows: Resume · Change class/hero (testing) · Cheat rows (6 toggles listed inline) · Aim assist · Team size · Reinforcements · FOV · Music volume · Mouse sensitivity · Invert Y · Low power resolution · Button prompts · Viewpoint · Respawn(→No/Yes) · Quit.
- No Options sub-page: settings live directly in the pause list (the original keeps them behind "Options").
- Selection indicator: bold label + panel highlight via `row_label` — no pulsing focus, no 9-slice cursor box.
- Sounds: Accept on click/confirm; Select on cursor move exists in shell only; in-game pointer accept was added in the uncommitted diff. No exit sound on Triangle, no cancel on popup No, no rate-limited select.
- No dim backdrop, no flashy title, no info-box help for most rows (help exists only for Assist/Team size/Reinforcements/InputIcons).
<br>
**Structural gap summary (the parity checklist):**
1. Menu structure: settings must move behind an "Options" row; add Restart Mission (dimmed until supported); rename title to "Game Menu".
2. Layout: width 260-unit stack, ySpacing 5, row pitch, centered at screen mid — currently free-form egui layout with `PANEL_MARGINS=60` heuristic.
3. Colors/fonts: yellow title, blue help text, grey (110) dimmed rows, gamefont-matched sizes/metrics.
4. Selection indicator: solid 9-slice cursor box (listbox_cursor), no pulse on list cursors (menuref §9.1.9); OpenBF has neither skin nor a defined focus visual spec.
5. Transitions/animations: flashy title, info text random delay, dim backdrop; no whoosh (correct to omit).
6. Navigation behavior: `bAcceptIsSelect` semantics, Triangle = resume+exit sound, popup No focuses first, L1/R1 page ±10 on sliders (the original IA options use L1/R1 = ±10), silent cursor at list ends vs wrap (stacks wrap and always click).
7. Sounds: full `MenuSound` set wired into pause (select/exit/cancel/error) with the 0.1 s select interval, using existing `shell_feel::Cues`.
8. Controller/keyboard/pointer: current scheme is close; add L1/R1 paging, Triangle-exit sound, popup focus rules.
<br>
## 4. Screenshot references needed
<br>
To match spacing/colors pixel-accurately, capture from the original (player-supplied; menuref captures referenced in `re-bf2-menus.md` already cover profile pick, IA, session options, keyboard, pause_menu text):
- Pause menu over Hoth/Endor ground battle (title, row list, focus box position) — checklist item 11 in `re-bf2-menus.md` §10 is exactly this capture.
- Pause → Options sub-stack; Game Options in-game centered page; Audio Options slider rows.
- Respawn/Quit Yes-No popups (focus on No), dim backdrop appearance.
- Slider row with value + focus box at min/max clamp, L1/R1 paging.
- Split-screen pause (`ifs_pausemenu1..3`, y −30 offset) for later splitscreen parity.
<br>
## 5. Asset requirements
<br>
Currently placeholders: the black rounded panel, egui default font, plain-text selection. Required new assets (all from player-owned game data, never in Git per project rules; bake pipeline via `scripts/`):
- `border_3_pieces` 9-slice window skin + `bf2_buttons_topleft/title_center/topright` title-bar pieces.
- `listbox_cursor` focus box 9-slice.
- `gamefont_tiny/small/large` glyph metrics (extracted as data, not art files in Git; rendering uses closest available font faces with matched advance/line heights).
- Menu click samples: **already extracted** (`bf2_CORE.snd.json`, CORE.LVL bank `global`) — no new assets, only wiring.
- Input icons: already implemented (`input_icons.rs`).
<br>
## 6. Risks
<br>
- **License/IP**: all skins/fonts/samples come from player data at runtime; no copies in repo (project rule). Evidence prose stays in our own words.
- **Determinism**: Time Scale must not touch `TICK_DT`; only the accumulator rate. Audio pitch and render blend must key off the same factor (`sfx_rate` pattern already exists for `slow`).
- **Shell coupling**: shell screens own `shell_feel` state; reusing `Cues`/`Pulse` in-game needs a per-battle instance — low risk, pure state.
- **Existing functionality preservation**: class picker (testing), respawn confirm, population rows, help lines must survive the restructure into Options pages; pointer click dispatch added by the uncommitted diff must keep working.
- **GPU/screenshot tests** require `scripts/gpu_tests.sh --test-threads=1` for visual checks.
<br>
## 7. Estimated complexity
<br>
Overall: **medium-large**, ~2,000–2,600 LoC (incl. tests), 6 phases, one PR each. Framework verdict: egui can reproduce layout, spacing, colors, fonts (metrics-matched), focus box, sounds and animations essentially faithfully via custom painter + the existing `shell_feel` kit; the only true gaps are dim backdrop (needs render-layer tint) and texture skinning (needs asset loading) — both additive, neither blocks parity of structure/navigation/sound.
<br>
## 8. Ordered implementation phases
<br>
**Phase 1 — Land the pending navigation/pointer/sound diff** (~265 LoC, low difficulty). Files: `menu.rs`, `pointer_navigation_tests.rs`, `events.rs`, `front.rs` (already done & validated). Testing: `cargo test -p ui --lib menu::` (20 tests), manual pad/pointer check.
<br>
**Phase 2 — Sound & feel wiring** (~150 LoC + tests, low–medium). Wire `shell_feel::Cues` into the in-game pause: Select on cursor moves with 0.1 s interval, Exit on Triangle-resume, Cancel on popup No, Accept on open, Error on refused (net-gate style) presses. Files: `menu.rs` (return cues with actions), `pause.rs`, `events.rs`. Testing: unit tests that cue sequences match menuref captures; in-game listen check.
<br>
**Phase 3 — Structure & layout parity** (~400 LoC + tests, medium). Title "Game Menu"; stack width 260 units, ySpacing 5, row pitch from gamefont metrics; settings rows move behind an **Options** row opening the pause sub-stack (Game Options / Audio Options / Controls pages reusing `MenuModel.in_shell` machinery); Respawn/Quit popups with focus-No and exact texts ("Do you really wish to kill your current player?" exists already); Restart Mission dimmed. Files: `menu.rs`, `pause.rs`, tests. Testing: row-list tests (existing "whole rows at any window" suite extended), navigation wrap/silent-at-ends tests.
<br>
**Phase 4 — Visual skin** (~350 LoC, medium–high). Load baked 9-slice skins + font faces; custom painter for window frame, yellow title bar with `flashy`, blue info box (random start delay 0–0.5 s), `listbox_cursor` focus box, grey-110 dimmed rows skipped by focus; dim backdrop tint on the battle render. Files: new `ui/src/skin.rs`, `menu.rs`, render-side tint hook, asset bake scripts. Testing: headless egui snapshot tests via `scripts/gpu_tests.sh --test-threads=1`; screenshot comparisons against Phase 0 references.
<br>
**Phase 5 — Cheats section (native-feeling)** (~200 LoC, low–medium). Move the 6 cheat toggles out of the main list into a **"Cheats"** row inside the Options sub-stack (same window skin, same row font, title "Cheats" in `game.pause` style text keys) — plausibly shipped as a hidden retail feature, consistent with Morse codes being the "real" BF2 cheat UI. Add **Time Scale** as a slider row (0.25×/0.5×/1×/2×/4×, Left/Right ±1 step, L1/R1 ±… stepping through the 5 stops; label `Speed  ◀ ▶` matching original value-row style): implement by scaling ticks-per-second in `WalkSim::advance_with` accumulator feed and keying sound pitch + render blend off it (`core` untouched; `[ours]` measurement tags per project rule). Future cheats slot in as more rows/words in the same page. Files: `menu.rs`, `cheats.rs`, `state.rs` (a `time_scale` field on session, not sim flags), `walk.rs` (accumulator scaling), `events.rs`. Testing: slider step tests, tick-rate unit test (ticks stepped per simulated second at each scale), determinism hash check (`--sim-hash`).
<br>
**Phase 6 — Polish & acceptance** (~150 LoC + docs, low). Split-screen pause offsets (y −30), net-game host-only cheat gating UI, help texts, `analysis` doc update, black-box F9 verification pass. Testing: full `cargo test -p ui --lib`, `cargo check -p game`, GPU tests headless.
<br>
## 9. Recommended PR breakdown
<br>
1. **PR A**: pending pointer/slider/sound diff (Phase 1) — ready to stage by name now.
2. **PR B**: pause menu cue wiring (Phase 2).
3. **PR C**: structure/layout restructure + Options sub-stack + popups (Phase 3).
4. **PR D**: skin assets, fonts, focus box, dim backdrop (Phase 4).
5. **PR E**: Cheats page + Time Scale (Phase 5).
6. **PR F**: polish/acceptance + evidence notes (Phase 6).
<br>
Per project rules: one feature per branch/PR, reserve work in the GitHub issues tracker, stage files by name, focused checks recorded with commands + revision + counts, no game data in Git.
<br>
---
<br>
**End of plan.** No code was modified in this session; all findings are read-only analysis. When you're ready to implement, toggle to **Act mode** and I'll start with PR A (staging the validated pending diff) followed by Phase 2.