# OPUS RE (2026-10-20) - [O] UNAUDITED
# The BF2 PS2 shell: its screens, flow, look, fonts, background movie and loading screen (card BSHL)

Card BSHL (lane 1; two sessions, the first died at its usage limit after its verifier round). Primary sources, PS2
only - the PC shell differs, so no PC layout is used here:
- **the disc's compiled shell scripts**: `SHELL.LVL` holds 92 `scr_` chunks (Lua 5.0 bytecode, extracted to
  `extracted/bf2/lua/shell/`), `COMMON.LVL` 42 more (`extracted/bf2/lua/common/`). Read with the Lua 5.0 disassembler
  of card TL (`engine/artifacts/lead_tmp/re/timelimit/lua50dis.py`, RK base 128; listings of all 134 in
  `engine/artifacts/lead_tmp/re/bshl/dis/`) and a reading-aid lifter (`bshl/lift.py` -> `bshl/lift/`, pseudo-Lua
  with each statement's pc; it garbles some registers, so every citation below is a pc of the RAW listing). A claim is
  cited `file proto pc`.
- **the disc's data**: chunk census (`bshl/census.py`), the movie config `mcfg` and the loading config `load`
  (`bshl/mcfg2.py`, `cfgdump.py`; names by FNV-1a over `byte | 0x20`), `MOVIES/SHELL.MVS` (copied to `bshl/mvs/`, cut
  into its 34 segments), the textures (`bake tex-dump` -> `bshl/tex/`), the English text table
  (`lead_tmp/re/evmsg/locl.py`; 1,310 shell keys in `bshl/keys_resolved.tsv`).
- **the ELF** (`analysis/SWBFRNT2.ELF-decomp.c`) for the screen size and the attract-mode flag, and **PCSX2**: a cold
  boot to the title, the profile flow, the main / Singleplayer / Instant Action / Session Options / Options screens and
  one load into Hoth (frames + five save states in `engine/artifacts/lead_tmp/re/bshl/ref_frames/`, index there).
Tags: [V] read (bytecode pc / bytes / decomp line cited); [D] derived; [H] hypothesis; [M] measured on a PCSX2 frame
or its RAM. `[O]` = one Opus pass + the verifier workflows of §11, no audit yet. Scratch: `engine/artifacts/lead_tmp/re/bshl/`.
Units: the PS2 shell lays out in a 512 x 448 screen (§3.5); a PCSX2 frame shows it at 640 x 480, so 1 unit = 1.25 px
across and 480/448 px down.

## 0. The answer

| question | answer |
|---|---|
| What is the PS2 shell made of? | Lua screens (SHELL.LVL / COMMON.LVL `scr_`) drawn with the engine's 2-D widgets: **one vertical-button stack style** for every menu, the BF2 **bitmap fonts** of CORE.LVL (`gamefont_large` for menu buttons), pieces from COMMON.LVL (`bf2_buttons_*`, the pad icons `btna` / `btnb`), art from SHELL.LVL (`logo`, `iface_instant_action`, icons) and a **full-screen background movie** (`MOVIES/SHELL.MVS`, MPEG-2 512x448) whose named properties the screens pick. [V][O] §1-§4 |
| The flow? | boot -> legal / logos -> **Press START** (only Start; 30 s idle -> the attract MOVIE) -> profile (pick / create, name, difficulty Normal / Elite, save?) -> **Main Menu: Singleplayer, Multiplayer, Splitscreen, Options** -> Singleplayer: Training, Space Overview, Rise of the Empire, Galactic Conquest, **Instant Action** -> the **Instant Action screen** (Planet -> Mode -> Era -> Playlist; LAUNCH / ADD / REMOVE / OPTIONS) -> Session Options -> LAUNCH -> the **BF2 loading screen** -> in the mission "Select a team" (`analysis/re-spawn-flow.md`). [V][M][O] §2 |
| The greyed-out Multiplayer (the maintainer, 2026-09-30)? | The PS2 has its own disabled style: a `bDimmed` button's label is drawn at grey 110 instead of 255 and **focus skips it** (`FollowButtonLink`). The PS2 uses it for Splitscreen with one pad (captured: Down from Multiplayer lands on Options). The port's Multiplayer takes exactly that. [V][M][O] §3.3 |
| Layout constants? | PS2 palette and pads in `interface_util` (title 246,235,20 a 0.8; focused 138,160,244 a 0.6; others 240 a 0.6; width pad 100, height pad 6, gutter 4; stacks at 0.6 of the height); rows `font height + 6`, pitch `+ 4` (main menu 22 + 6 + 4 = 32 units, **measured 31.97**); measured: labels drawn 1.087 x their font advances across, the bar = widest label + 134.2 units, left-justified stacks' bar edge at x 34.4. [V][M][O] §3 |
| The background? | Not art: the **movie**. Main-menu screens play `shell_main` (2 segments, 35 s + 32 s: film clips in two half-circle vignettes at the left and right edges, letterboxed); sub-menus play `shell_sub_left` (8 segments of ~10 s: one circle on the right, the stack on the left). The menu segments have no sound. The Instant Action screen plays no movie: a static backdrop `iface_instant_action` (its top 448 rows). [V][M][O] §4, §5 |
| Loading screens? | BF2 maps: a data-driven screen (LOAD.LVL's `load` config): the map's still, map name and mode across the top, a centred **Tips and Tricks** box (`loadtipsbox_pieces`, `level.common.tips.1..54`), `Loading` + dots. The port draws only the still and a caption. [V][M][O] §6 |
| What the port lacks | the shell art, the button pieces, the movie, the shell music, the loading config: none of them is baked (receipt). Fonts and the text table are. [V][O] §8 |

## 1. Where the shell lives on the disc [V][O]
Chunk census (`census.py`; listings `bshl/census_*.txt`):

| LVL | what | used for |
|---|---|---|
| `SHELL.LVL` (1,730,720 B) | 92 `scr_`, 56 `tex_`, 7 `BIN_`, 2 `mcfg` | the screens' Lua; `logo` 512x256, `iface_instant_action` 512x512 (the IA backdrop), `iface_bgmeta_space`, mode icons (`mode_icon_con/ctf/2ctf/hunt/ass/eli`), `mapselect_icon_*`, `seal_{all,imp,rep,cis}`, fleet icons, `planetgraphic_*`, `shell_tab`, the legal / logo screens (`bootesrb`, `_localize_<lang>_bootlegal`, `lucasarts_logo`, `pandemic_logo`, `dolby_logo`); the movie config (§4) |
| `SHELLPS2.LVL` | 3 `BIN_` only (`qdnaslib_scea/scee/scei`) | network (DNAS) - **no art, no script** [receipt:da9a4f6e] (control: SHELL.LVL 56 tex_, 92 scr_ [receipt:bcebdbc6]) |
| `COMMON.LVL` | 42 `scr_`, 80 `tex_` | the widget scripts (`ifelem_*`, `interface_util`, popups, options, pause menu); the 64x64 pieces `bf2_buttons_{topleft,title_center,topright,midleft,items_center,midright,botleft,botright}`, the 32x32 pad icons `btna` (Cross) / `btnb` (Triangle) / `btn_*`, `border_3_pieces`, `headerbutton*`, `listbox_*`, `slider_*`, `opaque_*`, faction icons |
| `CORE.LVL` | 4 `font`, 6 `Locl` | `gamefont_{tiny,small,medium,large}` (line heights 15 / 18 / 18 / 22; `analysis/re-hud.md` §1.1), the text tables |
| `MOVIES/SHELL.MVS` (272,652,288 B) | 34 MPEG-2 segments | the background movie (§4) |
| `SOUND/SHELL.LVL` | streams `shell_music`, `shell_vo`; bank `shell` | the shell music (`shell_soundtrack`), the menu clicks (`analysis/re-sound-banks.md`) |
| `LOAD/LOAD.LVL` | `load` config + `loadtipsbox_pieces`, `loading` | the BF2 loading screen (§6); stills per map in `LOAD/<MAP>.LVL` |
| `INSHELL.LVL` | torpedo weapon, `shell_cube`, `human_0..4` banks, sfx textures | not shell art [H: not followed] |

## 2. The flow on the PS2 [V][M][O]
`shell_interface.luac` main sets `gMovieStream = 'movies\shell.mvs'` (`shellpal.mvs` when `ScriptCB_IsPAL()`; pc 16-30),
pushes `ifs_boot` unless a campaign / metagame state is saved (pc 523-555; pc 505-522 first test two demo-build flags
that retail never sets), reads `sound\shell.lvl`, opens the streams `shell_vo` / `shell_music` and
`ScriptCB_OpenMovie(gMovieStream, '')` (pc 556-574). Measured on a cold boot (`ref_frames/boot_sequence_sheet.png`):
a loading screen (logo + dots), the legal list, Press START.

| # | screen (script) | what it shows | input -> next |
|---|---|---|---|
| 1 | `ifs_boot` | nothing (memory-card / controller check) | -> `ifs_legal` (main.f0 pc 36-38); on the PS2 it first waits while no pad is connected (pc 8-13) and may push `ifs_saveop` for the memory-card check (pc 14-35) |
| 2 | `ifs_legal` | `gLegalScreenList` in order: `bootESRB` 4 s (skipped on PAL), `bootlegal` 5 s, `lucasarts_logo` 3 s, `pandemic_logo` 3 s, (`nvidia_logo`: `bNoPS2`, skipped), `dolby_logo` 3 s; the logos skippable (main pc 0-27; main.f0 pc 7-71) | after the last -> `ifs_start` |
| 3 | `ifs_start` | `logo` (512x256 at the top, y -38), `shell_main` movie, "Press START Button To Begin" (`gamefont_large`, 210 units above the bottom, pulsing alpha 0.4..1 at 0.5/s), music `shell_soundtrack` (main pc 0-71; main.f2 pc 45-64) | **only Start** advances (main.f8; Accept and the four directions return at once, main.f3-f7) -> profile. **30 s idle** (main.f2 pc 65-79) -> main.f17: the engine has set `gMovieAttractMode`, so it pushes **`ifs_attract`**: the movie property `attractmovie` full-screen with "Star Wars Battlefront II / Press any button to exit." (`gamefont_large`, 0.9 down); any button (face, shoulder, D-pad, Start, Select; the four `Input_General*` handlers are empty) or 180 s pops back (ifs_attract main.f0, f1 pc 13, f3-f18, f23). The flag: `FUN_00428128` sets the Lua global true when byte `DAT_00600b97` != 0 (B2 483898-483903); the byte has no writer [receipt:6a53184d] and is 1 in the ELF, so `ScriptCB_LaunchDemo` / `attract_mode_maps` are not reached on the PS2 (§11 D3). The movie (segments 32-33, ~69 s each, silent) is a gameplay montage, one Clone Wars, one GCW (`mvs/sheet_attract.png`). |
| 4 | `ifs_login`, `ifs_vkeyboard`, `ifs_difficulty`, save popup | Pick a Profile (list + Create), the name keyboard, **Select Difficulty: Normal / Elite** (two buttons only, `ifs_difficulty` main pc 20-37), "save Player 1 to the memory card?" | -> `ifs_main` |
| 5 | `ifs_main` | logo + stack "Main Menu": **Singleplayer, Multiplayer, Splitscreen, Options** (the layout lists 8 buttons; on the PS2 `quit`, `back2` (Profiles), `tutorials`, `quitdemo` are hidden; main.f15 pc 146-205); Splitscreen dimmed while fewer than 2 pads (main.f17, checked every 0.1 s when the count changes) | sp -> `ifs_sp`; mp -> `ifs_mp` (a "no network adaptor" popup without one); split -> `ifs_split_profile`; opts -> `ifs_opt_top` (main.f19). Port: `[ours]` shows the PC's `quit` row last, labelled "Quit Game" (the disc's "Exit To Windows" is wrong off Windows); it exits |
| 6 | `ifs_sp` | stack "Singleplayer", left-justified, `shell_sub_left` movie: **Training, Space Overview, Rise of the Empire, Galactic Conquest, Instant Action** (Training hidden in splitscreen, main.f3) | ia -> rules `instantaction` -> `ifs_instant_top`; riseempire -> (training question) `ifs_freeform_rise_newload`; meta -> `ifs_sp_campaign`; spacetraining -> `ifs_spacetraining`; training -> mission `geo1c_c` (main.f8) |
| 7 | `ifs_instant_top` | (none; a router) | -> `ifs_missionselect` with rules `instantaction` (main.f2 pc 0-27) |
| 8 | `ifs_missionselect` | the Instant Action screen (§5) | OPTIONS -> `ifs_instant_options_overview` -> `ifs_instant_options`; LAUNCH -> (a map selected) `ifs_instant_side` (instant_top main.f0) |
| 9 | `ifs_instant_side` | nothing: stops the movie, `ScriptCB_SetTeamNames(Team1Name, Team2Name)`, `ScriptCB_EnterMission()` (main.f0) [V lift, read after the verifier pass: lead-only] | -> the BF2 loading screen (§6) -> the mission's side select (`re-spawn-flow.md`) |
| - | `ifs_opt_top` | stack "Options" (left-justified, `shell_sub_left`): Game Options, Audio Options, Controls, Online Options, View Trailer, Credits, Career Stats [M] | |

Screen changes go through `ifs_movietrans_PushScreen`; no shell or common script gives a screen a `movieIntro` (every
assignment stores nil; §11 D5), so the change itself plays no intro and starts no movie (ifs_movietrans main.f3 passes
nil to `fnStartMovie` when there is no intro); the next screen's Enter (template main.f10 pc 61-66) starts its
`movieBackground` and sets its music [V].

## 3. The look: one vertical-button stack [V][M][O]
### 3.1 The PS2 palette and pads - `interface_util.luac` main pc 3-54 (the `gPlatformStr == 'PS2'` branch)
| name | PS2 value |
|---|---|
| gTitleTextColor / alpha | (246, 235, 20) / 0.8 |
| gSelectedTextColor / alpha | (138, 160, 244) / 0.6 |
| gUnselectedTextColor / alpha | (240, 240, 240) / 0.6 |
| gHelptextTextColor / alpha | (240, 240, 240) / 0.8 |
| gButtonWidthPad, gButtonHeightPad, gButtonGutter | 100, 6, 4 |
| gPopupWidthPad, gPopupHeightPad | 32, 32 |
| gDefaultButtonScreenRelativeY | 0.6 |
| gMenuButtonFont, gListboxItemFont | `gamefont_large` |
| gPopupTextFont, gPopupButtonFont | `gamefont_small` |

### 3.2 A stack - `ifelem_button.luac` AddVerticalButtons (main.f6) / ShowHideVerticalButtons (main.f7)
Code [V]:
- Row height = `ScriptCB_GetFontHeight(font) + gButtonHeightPad`, spacing `gButtonGutter` (f6 pc 30-42). A layout
  without `HardWidthMax` gets `safe width - 16` (f6 pc 23-29); `maxbgwidth` = the largest button / layout `width`
  (default 100; f6 pc 43-95).
- Width = the largest, over the visible buttons and the title, of (the label's display-rect width +
  `gButtonWidthPad`), capped at `HardWidthMax` (f7 pc 36-64, 88-98, 105-133).
- Vertical: with a title, the title bar sits at y = -(n+1)(h+s)/2 (n visible buttons), the rows follow 11 units lower,
  one per h + s plus a button's optional `yAdd` (f7 pc 66-104, 240-265; no menu screen of §2 uses `yAdd`); the stack's
  container sits at `ScreenRelativeY` 0.6 of the screen (the main menu's 10 units lower).
- The title row draws over `bf2_buttons_topleft / title_center / topright` (blue, rounded top); every button but the
  last over `bf2_buttons_midleft / items_center / midright` (grey, notched); the last over `bf2_buttons_botleft /
  items_center / botright` (rounded bottom) (f6 pc 97-148, f7 pc 183-239). "Last" is the last of the layout's list,
  hidden or not. Links wrap last <-> first (f6 pc 276-303).
- "Flashy" text (layouts whose `flashy` is not 0): each visible row's label and bar start after (row number) x 0.1 s
  (f7 pc 180-239) [V; the effect itself is engine code, not read].
- Left-justified layouts (`bLeftJustifyButtons`: Singleplayer, Options) move the container to
  x = (maxbgwidth - width) x -0.5 + 20 (f7 pc 291-304).
- Focus moves play `shell_select_change` (f3 pc 36-49); the template's default sounds are accept `shell_menu_accept`,
  back `shell_menu_cancel`, exit `shell_menu_exit`, error `shell_menu_error`, enter none (`ifelem_shellscreen`
  main.f30 pc 13-32 and on).
Measured [M] (`bshl/meas/`, frames at 1.25 px per unit):
- Row pitch on the main menu: dark gaps at y 238, 272, 307, 341, 375 of 480 = 34.25 px = **31.97 units** (22 + 6 + 4).
- Labels are drawn **1.087 x the font's advance sum across** (six Options labels: 1.07-1.09) and about 1:1 down.
- Bar outer width = 1.087 x (the widest label's advance sum) + **134.2 units** (main menu 285.6-286.0, Singleplayer
  349.2, Options 301.6; spread 0.9; three points, a weak fit): `gButtonWidthPad` 100 plus ~17 per side.
- Centred stacks centre on the screen (main menu 258 of 512); both left-justified stacks put the bar's **left edge at
  x 34.4 units** (43 px) = the safe area's left edge 25.6 + 8.8, whatever their width.
- [H] The code route to that edge is open: with `maxbgwidth` = the layouts' `width` (Singleplayer 300, Options 360)
  the formula predicts the two stacks' centres at ~283 and ~230 units, i.e. different left edges, but the frames show
  one left edge (centres 209 and 185). A container x measured from another origin, or a display-rect width that is not
  the ink width, could explain it; the engine side (`IFObj_fnSetPos` on a `ScreenRelativeX` container, the flashy
  text's box) is not read. The build pins the MEASURED rule with a pixel test, not the formula.

### 3.3 Dimmed (disabled) buttons
`ShowHideVerticalButtons` draws a `bDimmed` button's label at 110 instead of 255 (f7 pc 266-289) and
`FollowButtonLink` (f3 pc 0-52) walks past any linked button that is hidden, `bDimmed` or invisible, so focus never
lands on one (the first focus `TagOfFirst` skips dimmed ones too). PS2 uses: Splitscreen below 2 pads (`ifs_main`
main.f17). **Captured**: with one pad the main menu shows Splitscreen grey, and Down from Multiplayer lands on Options
(`ref_frames/main_menu_mp_focus.png` -> `main_menu_opts_focus.png`). [V][M][O]

### 3.4 The screen template - `ifelem_shellscreen.luac`
- Help text (non-PC, main.f14 pc 79-124): bottom-left "Back" with `btnb` (Triangle), bottom-right "Accept" with `btna`
  (Cross), "Select" when the screen sets `bAcceptIsSelect`; both 15 units above the bottom of the safe area;
  `gHelptextTextColor` 240 a 0.8; a screen may drop either (`bNohelptext`, `_back`, `_accept`).
- Enter (main.f10 pc 49-66): sets the screen's shell music and starts its `movieBackground` looping.
- A screen with `bg_texture` gets a full-screen image of it: 0..w x 0..h with the bottom v = the screen aspect 0.875
  (main.f14 pc 0-24), so a 512x512 texture shows its top 448 rows; otherwise no template background (pc 25).
- `bDimBackdrop` (Options and the in-game option pages) adds an `opaque_black` image at ZPos 255 (main.f14 pc 26-58);
  in the shell the movie still shows through on Options [M].
- The logo screens (`ifs_start`, `ifs_main`) draw `logo` 512x256 centred at the top, 38 units above the top edge.

### 3.5 The shell's screen space [V][M][O]
`FUN_00580b08` (B2 715826-715829) sets the screen record at 0x7d5f70 to **512 x 448**, `DAT_0060c7ac` = **0.875**
(the ELF image holds 1.0; this is its one writer) and the safe scale at 0x7d5f80 = **0.9**.
`ScriptCB_GetScreenInfo` returns (w, h, 0.875, 1.0) and `ScriptCB_GetSafeScreenInfo` (w x 0.9, h x 0.9, 1.0)
(B2 28243-28270). The shell's RAM holds the same (states `bf2_shell_main` / `_ia`). Safe area 460.8 x 403.2 from
(25.6, 22.4). PCSX2 shows the 512x448 frame at 4:3.

## 4. The background movie [V][M][O]
- `SHELL.MVS` = a `ucfb` whose index at 0x20 opens with a header record (count 34) and holds 34 records of 8 words
  from 0x38 (`'name'` 0x8d39bde6: hash, `'length'` 0x83d03615: bytes, ...); segment k starts at 0x800 + the earlier
  lengths each rounded up to 2048 and starts with an MPEG pack header; the last ends at 272,650,244, padded to the
  file's 272,652,288 (`bshl/checks/mvs_index_recheck.txt`). Every segment is **MPEG-2 video 512x448 at 29.97 fps**.
  Segments 10-31 also carry Sony PSS audio (private stream 1, `SShd` header, 48 kHz stereo; ffprobe does not list
  it); **segments 0-9 (the menu loops) and 32-33 (the attract movie) carry none**. [V][M]
- SHELL.LVL's movie config (`mcfg` 0x12d97623, `bshl/mcfg_shell.txt`): three fade-only entries, then the
  **properties**: `shell_main` = segments 0-1 (34.97 s + 32.07 s; a field 0x4d5af670 = 1 [H: random or sequential
  pick]); `shell_sub_left` = segments 2-9 in the order 2,3,4,5,8,6,7,9 (8 loops of 9.4-10.0 s); 18 `<map>tran1`
  properties (cor1 dea1 fel1 geo1 hot1 kam1 kas2 mus1 myg1 nab1 pol1 sb1-sb4 tan1 uta1 yav1; Lua names them only in
  `ifs_campaign_data`, the Rise of the Empire campaign - Instant Action never plays them); 4 unnamed one-segment
  properties (segments 28-31); `attractmovie` (segments 32-33, also 0x4d5af670 = 1); `train`, `trail`. [V]
- What it looks like (`mvs/sheet_00.png`, `sheet_sub.png`, `f_pair.png`): film clips (both trilogies, Clone Wars and
  GCW) inside circular vignettes with the faction insignia over them. `shell_main`: two half-circles at the frame's
  left and right edges (the menu stack sits in the dark centre); `shell_sub_left`: one circle on the right half (the
  stack on the left). Both letterboxed inside the 448 lines. [M]
- `ifelem_shellscreen_fnStartMovie` (template main.f1): plays only when the property is not already playing; skips
  when `gMovieDisabled`, or under the `metagame` / `campaign` rules unless `gMovieAlwaysPlay` - a skipped movie is
  kept in `gPrevMovie` for `ifs_saveop` to restart later. Full screen.
- Which screens: `shell_main` - `ifs_start`, `ifs_login`, `ifs_main`, `ifs_mp`; `shell_sub_left` - `ifs_sp`,
  `ifs_opt_top`, `ifs_sp_campaign`, `ifs_split_profile`, `ifs_instant_options_overview`; none - `ifs_missionselect`,
  `ifs_difficulty`, `ifs_instant_side`; `attractmovie` - `ifs_attract` (§11 D6).

## 5. The Instant Action screen and Session Options [V][M][O]
- `ifs_missionselect`: `nologo`, `bg_texture = 'iface_instant_action'` (the metal panel with the centre housing and
  the red sign baked in; its top 448 rows fill the screen, §3.4 - measured: texture row r lands at r x 480/448 px),
  **no movie** (main pc 120-125). Planet -> Mode -> Era lists in the left window, the Playlist on the right
  (`NewButtonWindow` with title bars), the command stack LAUNCH / ADD / REMOVE / OPTIONS in the centre housing
  (`gamefont_tiny`, square buttons, `yTop` -130, width cap 80; main pc 67-87), a description box along the bottom
  (`gamefont_tiny`, focused-text blue).
- List layout (main.f34): font `gamefont_tiny` on the PS2; each list `safeW x 0.375 - 35` wide; rows `font height + 2`
  (English), the Era list's `2 x (font height + 4)`; rows shown `clamp(floor((safeH - 240) / (row + spacing)), 4, 16)`
  (9 on the PS2: CORUSCANT ... KAMINO, then a scroll arrow), the Era list half that.
- Measured flow (`ref_frames/ia_*.png`): focus starts on ADD; ADD moves into the Planet list (Coruscant ... Yavin 4,
  then "All Maps"); a planet -> Mode (Conquest / 1-Flag CTF / Hunt / All Modes) -> Era (only the eras the map has) ->
  the playlist gains "HOTH", focus LAUNCH; the info box shows the planet's name + description or the focused
  command's help ("LAUNCH THE CURRENT PLAYLIST.").
- OPTIONS -> "Session Options" (Global, Hero, Conquest, CTF, Hunt, Assault) -> e.g. **Global Options**: AI Units Per
  Team < 16 >, AI Difficulty Level < Normal >; **Conquest Options**: AI Units Per Team < 16 >, Reinforcements < 100% >,
  Timer < Off >, each with a description box. The port's `ShellModel` holds these values already (`ai_per_team`,
  `ticket_pct`, `time_limit`; `analysis/re-time-limit.md`). [M]

## 6. The BF2 loading screen [V][M][O]
- `LOAD/LOAD.LVL` `load` config (`cfgdump.py`): tips prefix `level.common.tips` (Galactic Conquest: `level.gal1.tips`),
  the box `loadtipsbox_pieces` (12, 12), colours (155,171,207,210), (200,200,200,255), ..., the dots texture
  `loading`, and per map its stills LVL `load\<map>` with a count (Hoth 30) [H: the count's meaning; 9 root fields
  stay unnamed hashes]. The English table holds `level.common.tips.1` .. `.54` (each "Tips and Tricks:\r\n\r\n...")
  and `level.gal1.tips.1` .. `.21` [V]. Which tip shows, and in what order, is engine code not read [H].
- Measured (`ref_frames/loading_hot1_*.png`, shots 1.5 s apart): the map's still across the frame; "Hoth" top-left
  and "Conquest" top-right on black; a centred translucent box with the tip; "Loading" + 15 dots filling bottom-left;
  the tip changed once in a ~13 s load. [M]
- The port (`ui/src/loading.rs`): the pool's still pillarboxed + "<caption>   n/N" in light blue - no header, no tips
  box, no dots. BF1 maps keep BF1's zoom sequence (PLAN §Presentation).

## 7. PCSX2 reference set [M]
`engine/artifacts/lead_tmp/re/bshl/ref_frames/` (index.md lists every frame) + five save states (title, main menu,
Singleplayer, Instant Action, Options). Not copied into `engine/artifacts/pcsx2/ref/` (the oracle guard keeps that
library for capture jobs; a planner may promote the set). Pad taps in the shell need a 300 ms hold and 1.5-1.8 s
between presses.

## 8. What the port has and lacks [V][O]
Baked today: the four fonts (`common_bf2/fonts/`), the English text table (`common_bf2/locl/`), `border_3_pieces`,
`opaque_black`, the BF2 load stills. **Not baked** - none of `iface_instant_action`, `bf2_buttons_topleft`,
`loadtipsbox_pieces`, `shell_soundtrack` occurs in `engine/assets_baked` (22,869 files) [receipt:854c8551] (control:
`border_3_pieces` 14 hits, `gamefont_large` 21 [receipt:6e8379ac]); so SHELL.LVL's textures, the button pieces, the
movie, the shell music and LOAD.LVL's config all need a bake. Today's shell (`ui/src/shell.rs`) is one egui list of
nine rows (map, mode, two factions, AI, reinforcements, timer, Start, Quit).

## 9. The port's content - the calls (defaults, each reversible)
The look and the order stay the PS2's; the content follows the port's features (PLAN §Presentation, §Milestones).
1. **Legal / logo screens: skipped** (publisher legal of a 2005 retail disc; modern QoL). Reverse: the textures are
   on the disc (§1); `ifs_legal`'s list and times are in §2.
2. **Title (Press START): kept** - logo, `shell_main`, the pulsing prompt; Start on a pad, Enter on a keyboard (the
   PS2 takes Start only). The attract movie after 30 s idle: kept if the movie is baked (it is two segments of the
   same file), else not built.
3. **Profile flow:** Start opens the named profile list. Create opens the name keyboard and then difficulty.
   The port offers its four difficulty tiers and autosaves the profile. Main Menu Back returns to the list.
   See Saves, sessions & host authority for progression and multiplayer ownership.
4. **Main Menu**: Singleplayer; **Multiplayer in its PS2 slot, dimmed (`enabled: false` in data), skipped by focus**
   (§3.3 - the PS2's own disabled style, so the maintainer's "focus skips it or confirm does nothing" is settled as "focus
   skips it", FEEL_NOTES line in the build); Splitscreen dimmed until SSCR; Options.
5. **Singleplayer**: Instant Action live; Rise of the Empire and Galactic Conquest dimmed until their M5 cards; Training
   and Space Overview hidden (no port mode behind them).
6. **Instant Action**: the PS2 screen with the port's maps in the Planet list (BF1 and BF2 by their names), the modes
   the port runs, the two eras plus the port's cross-era pairing (PLAN: era mix-and-match) as an Era row that opens a
   faction-pair list; Session Options -> Global / Conquest with the port's values; LAUNCH runs the playlist's first
   map. ERAM's two notes (army rows "Your army / Enemy", the mode label) go with it.
7. **Options**: the PS2 stack with the port's pages (Game, Audio, Controls, Video); Online / Trailer / Credits hidden;
   Career Stats when the profile shows it.
8. **Loading**: BF2's screen for BF2 maps (§6), BF1's zoom for BF1 maps (unchanged).
9. **Widescreen**: the movie and the IA backdrop are 4:3 art - pillarboxed, never stretched (COMPARISON_RULES); the
   stacks anchor to the centre (Hor+), text at the fonts' native texels scaled by display height / 448 (x 1.087
   across, §3.2).

## 10. Build cards
- **BSHLA** (value 5): bake the shell art + the movie's menu segments + music; the title, Main Menu, Singleplayer and
  Options stacks in the PS2 style (§3, §4, §9.1-5, 7), with the disabled Multiplayer.
- **BSHLB** (value 4, deps BSHLA): the Instant Action screen and Session Options (§5, §9.6), replacing `ui/src/shell.rs`.
- **LDBF2** (value 4): the BF2 loading screen for BF2 maps (§6).
- Notes on existing cards: UISN (the shell's clicks: `shell_select_change` per focus move, `shell_menu_accept`,
  `shell_menu_cancel`, `shell_menu_exit`); SSELA / SSELB (`ifs_instant_side` hands over to the mission's side select).

## 11. Checks run
**re_check.py** (outputs in `engine/artifacts/lead_tmp/re/bshl/checks/`):
- `fn b2 28248 28266 483898 483900 715826 715828 715829` -> ScriptCB_GetScreenInfo (0x185bb0), ScriptCB_GetSafeScreenInfo
  (0x185c90), FUN_00428128 (x2), FUN_00580b08 (x3).
- `word b2 0x636918 --str` -> 'gMovieAttractMode'; `word b2 0x600b94 1 --u32` -> 0x01000000 (byte 0x600b97 = 1);
  `word b2 0x60c7ac 1 --f32` -> 1.0 (the RAM holds 0.875 after FUN_00580b08).
- `writers b2 DAT_00600b97` -> 0; `writers b2 DAT_0060c7ac` -> 1 (B2:715828 in FUN_00580b08, = 0x3f600000);
  `callers b2 FUN_00428128` -> FUN_001758f8; `callers b2 FUN_00580b08` -> FUN_00174458.
- `absent`: SHELLPS2.LVL has no tex_ / scr_ / logo [receipt:da9a4f6e] (control SHELL.LVL [receipt:bcebdbc6]); the bake
  has none of the four shell names [receipt:854c8551] (control [receipt:6e8379ac]); nothing names 0x600b94..0x600b97
  but the one read [receipt:6a53184d].
- Lua bytecode has no decomp lines: every Lua claim cites `file proto pc` of the raw listing, re-disassembled by each
  verifier. Twin claims: none (this is BF2-only; BF1's shell is a different program and is not claimed).
- Own re-check of the movie index and audio (`checks/mvs_index_recheck.txt`): 34 records of 8 words from 0x38, all 34
  starts hold a pack header, the last segment ends at 272,650,244, private-stream-1 sectors in segments 10-31 only.

**Verifier workflows** (claims and citations only; every absence / flag claim to two readers who never saw each other):
- Round 1 (`bshl-verify`, the first session, 4 agents; `verify/claims.json` -> `verify/VERDICTS.json`): 24 claims,
  27 verdicts - 19 SUPPORTED, 5 PARTLY (C4: the attract branch - the readers found the engine sets
  `gMovieAttractMode`, now §2; C15: a skipped movie is kept in `gPrevMovie`; C17: 18 tran1 entries, not one per map;
  C18: the index header and padding; C19: segments 10-31 carry PSS audio that ffprobe hides - both readers), 0 NOT
  SUPPORTED. The second reader overturned nothing (C16, C22 SUPPORTED by both; C19 PARTLY by both).
- Round 2 (`bshl-verify-2`, this session, 4 agents; `verify/claims2.json` -> `verify/VERDICTS2.json`): 16 claims,
  20 verdicts - 12 SUPPORTED, 4 PARTLY (D1: ifs_boot also waits with no pad; D2: the four General handlers are empty;
  D5: with no intro the movie is started by the next screen's Enter, not by movietrans - both readers; D8: the
  optional `yAdd` and the `flashy` gate), 0 NOT SUPPORTED; the second reader overturned nothing (D3, D11, D12
  SUPPORTED by both, D5 PARTLY by both). The measurement reader re-measured D14 / D15 (main menu 285.6 units by the
  bright-outline convention; IA panel edges 64.3 / 413.6 px against 64.29 / 413.57 predicted).
- Total: 40 claims, 31 SUPPORTED, 9 PARTLY (each narrowed in the text above), 0 NOT SUPPORTED.

**Stays [H] or lead-only**: the stack's left-justify route (§3.2); the `0x4d5af670` movie field; the loading config's
per-map count and its 9 unnamed root fields; the tip choice / order; INSHELL.LVL; the flashy-text and help-text
engine code. `ifs_instant_side` (§2 row 9) and the bDimBackdrop / movie draw order (§3.4) were read after the
verifier pass: lead-only.
