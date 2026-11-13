# OPUS RE (2026-10-27) - [O] UNAUDITED
# The BF2 PS2 menus in full: Instant Action, Session Options, the rest of the tree, sounds, the focus pulse, the map list (card BF2MENUS)

Goal: everything needed to rebuild BF2's PS2 menus exactly, starting with Instant Action, with options the port has
not built shown in the PS2's own dimmed style; the one deliberate difference is an Instant Action map list holding
both games' maps, each name followed by "(bf1)" or "(bf2)". This note extends `analysis/re-bf2-shell.md` (read it
first: the flow, the palette, the vertical-button stack, the dimmed style, the screen template, the screen space).
It does not repeat that note.

Sources, PS2 only:
- the disc's compiled shell scripts (Lua 5.0 `scr_` chunks of SHELL.LVL / COMMON.LVL, `extracted/bf2/lua/`), read
  in the raw listings `engine/artifacts/lead_tmp/re/bshl/dis/<lvl>_<script>.txt` (`lua50dis.py`, RK base 128). A
  claim is cited `script proto pc` of the raw listing; the lifts in `bshl/lift/` were a reading aid only. BF1's
  scripts are Lua **4.0** (the 5.0 disassembler refuses them); §6 used a 4.0 lister written for this job
  (`engine/artifacts/lead_tmp/re/bf2menus/maps/l40.py` on `lead_tmp/audit-annc/lua40.py`).
- the English text tables (`lead_tmp/re/evmsg/locl.py`, `locl_bf2.tsv`; keys hash FNV-1a over `byte | 0x20`, so a
  key's `_` hashes as 0x7f), the LVL censuses (`bshl/census_*.txt`), the sound chunks (CORE.LVL bank `global`,
  SOUND/SHELL.LVL; parsed with `lead_tmp/s12/re/sound_scratch/snd_parse.py`).
- the ELF decomp `analysis/SWBFRNT2.ELF-decomp.c` (cited `B2 <line>`) for the native parts: the flashy-text draw and
  its timing table, `ScriptCB_GetNetGameDefaults`, the mission-queue size.
- the PCSX2 reference frames of card BSHL (`bshl/ref_frames/`, listed in its `index.md`); PCSX2 was NOT run for this
  note (§9 lists what needs it). A later PCSX2 walk with video and sound (2026-10-27/28,
  `engine/artifacts/lead_tmp/menuref/README.md`, cited "menuref" with its clip / frame / sheet) answered §9 and
  contradicted the note in places: those claims are corrected in place, tagged [M], and listed in §10.
Scratch (not tracked): `engine/artifacts/lead_tmp/re/bf2menus/` (`sec/` the section drafts and `assemble.py`, `maps/` the
BF1 listings and key lookups, `snd/` every sound call site `play2.tsv` and the resolved SHELL.LVL properties).
Tags: [V] read, [D] derived, [H] hypothesis, [M] seen on a named reference frame. `[O]` = one Opus pass: a lead and
four parallel readers (IA + Session Options by the lead; menu tree, sounds, focus pulse, map list by one reader each,
with the lead spot-checking the pulse bytecode `common_ifelem_roundbutton` main.f6 and `common_ifelem_listmanager`
main.f5). No verifier round yet. Units: the PS2 shell's 512 x 448 screen (re-bf2-shell §3.5).

## 0. The answer

| question | answer |
|---|---|
| Instant Action screen | `ifs_missionselect`: static backdrop `iface_instant_action`, no movie, no music change. Left window (Planet -> Mode -> Era, one at a time, 172.8 x 183 units, title bars "Planet" / "Mode" / "Era"), right window "Playlist" (same size), a square command stack LAUNCH / ADD / REMOVE / OPTIONS (`gamefont_tiny`) in the centre housing, a full-width info box (76 tall, blue text) along the bottom; lists in `gamefont_tiny`, 9 rows of 17 units (Era 4 rows of 38). Five states: command, map, mode, era, playlist. Cross walks Planet -> Mode -> Era and the Era pick adds the row and jumps to LAUNCH; Triangle walks back; L1 / R1 page; Square does nothing. Help bar: Triangle "Back", Cross "Select". Playlist starts as one row "Remove All"; LAUNCH is dimmed until a map is added; max 128 rows. [V][M] §1 |
| Session Options | `ifs_instant_options_overview` (stack "Session Options", 305 wide, `shell_sub_left`): Global, Hero, Conquest, CTF, Hunt, Assault (Host hidden in SP; no Elimination). Each opens `ifs_instant_options`: a 391.7-wide window of 5 rows of 40 (`gamefont_small`) "Label: < value >", Left / Right +-1 step, L1 / R1 +-10 steps, a grey info box with the row's help line, help bar Back only. Values: AI Units 0..16 default 16 [M menuref `so_global`]; AI Difficulty Normal <-> Elite; Reinforcements 10..500 % step 10 default 100; Conquest / CTF timer Off or 5..60 min step 5 default Off; CTF score 1..15 default 5; Hunt timer 5..60 default 5, score 10..1000 step 10 default 50; Assault score 50..1000 step 10 default 180; Hero rows (Heroes default **Off**, the other rows hidden while Off; Points <-> Timer default Points, value 1..120 default 10, Best -> Worst -> Random default Best, respawn Always / 1..120 s / Never default 90 s [M menuref `so_hero`, `so_hero_ranges`]). [V][M] §2 |
| The rest of the tree | Profile (Pick a Profile, keyboard 4 x 13, Select Difficulty, save popup), Main Menu, Singleplayer (Training, Space Overview, Rise of the Empire, Galactic Conquest, Instant Action), Multiplayer (PS2 network chain), Splitscreen, Options (Game Options 10 rows, Audio 5 sliders + Output Mode, Controls 5 modes + pad page, Online, View Trailer, Credits crawl, Career Stats) and the pause menu, each with its rows, values, setters and pad buttons. [V][M] §3 |
| Sounds | All clicks live in **CORE.LVL's bank `global`** (bus `shellfx` gain 0.4): focus move `shell_select_change` (63 ms, pitch 1.5, at most one per 0.1 s), accept `shell_menu_accept` (428 ms), back = the screen you return to plays `shell_menu_exit`, refusal `shell_menu_error` (= `shell_menu_cancel`'s sample `ui_cancel`), popup No `shell_menu_cancel`, name keys `shell_keyboard_key/backspace`. List moves click only when the cursor moves (silent at the ends); stacks wrap and always click; no dimmed-press sound (focus never lands on one). Option values click `shell_select_change` even at a clamp; Game Options rows play accept; volume sliders play a voice / sfx audition line, not a click. No transition whoosh in BF2 (`shell_transition1..10` exist but no screen has a `movieIntro`). The samples are baked; nothing in the port plays them. [V] §4 |
| The focus pulse | The focused button's whole row (bar + label) **pulses its alpha on a linear triangle 0.3 <-> 1.0 at 2.0 /s, stepped at the shell's 30 Hz: 11 updates each way, a 0.734 s (22-update) cycle** [M menuref `measure/main_idle_pulse.csv`], continuously while focused, one global phase shared by all buttons and not reset on a move; colours step at once (focused label (138,160,244), bar white; others label 240 a 0.6, bar grey 150). **A list's `listbox_cursor` box does not pulse** on the PS2 (static for 5 s beside a pulsing ADD, [M] menuref `measure/ia_list_pulse.csv`); row text is static. Every screen Enter plays a one-shot "flashy" entrance per text, measured at bar wipe ~0.07 s, square ~0.1 s, text pop ~0.07 s, fade ~0.07 s [M menuref `measure/flashy_sp_entrance_strip.png`] (the ELF table says 0.1 / 0.05 / 0.05 / 0.15 / 0.15), menu rows staggered 0.1 s, list rows at random 0..0.5 s. [V][D][M] §5 |
| Map list | BF2: 24 maps + "All Maps", sorted by English name (Coruscant ... Yavin 4, six space maps among them), names / descriptions `mapname.name/description.<code>`, modes per map and era, eras Clone Wars / Galactic Civil War (+ "All Modes" / "All Eras" when more than one). No preview on the PS2 IA. BF1: 16 maps, conquest only, names `planets.<p>.mapname<n>` ("HOTH: ECHO BASE"), no description, preview clips `<CODE>fly`. Merged list of 40 + All Maps in §6 (sort case-folded; 8 codes exist in both games, so key by `<game>_<code>`). [V][M] §6 |
| What the bake lacks | `iface_instant_action`, `mode_icon_*`, BF2 copies of the era icons, `listbox_cursor`, `listbox_topbot_more`, `white_rect`, `cursor`, `btnmisc*`, slider sheets; the 6 space maps and every non-conquest mission. Fonts, text (both games), button pieces, `btna/btnb`, `optionl/optionr`, `border_3_pieces`, the menu movies, the music and the click samples are baked; the clicks are not wired. §8 |
| Open | answered by the PCSX2 walk (§10): AI cap 16, Hero defaults, the info-box font (the rows' `gamefont_small`), the music (a random take each time), the timings. Still open: the column-change fade, the pulse's phase carry-over, BF1 (§9). |

## 1. The Instant Action screen (`ifs_missionselect`)

Script `shell/ifs_missionselect` (listing `dis/shell_ifs_missionselect.txt`). Reached from Singleplayer -> Instant
Action through the router `ifs_instant_top` (Enter: `ScriptCB_SetDifficulty(ScriptCB_GetDifficulty())`,
`fnDone = ifs_instant_fnSelectDone`, `bForMP = nil`, `ScriptCB_SetGameRules('instantaction')`,
`ScriptCB_SetIFScreen('ifs_missionselect')`, `ifs_instant_top` main.f2 pc 0-23) [V]. Because it is *set*, not pushed,
Back from the IA screen pops straight to Singleplayer [D].

### 1.1 The screen record [V]
`NewIFShellScreen{nologo=1, bAcceptIsSelect=1, bg_texture='iface_instant_action', movieBackground=nil, iColumn=0,
iState=map, bRandomOrder=nil, buttons=container(ScreenRelativeX 0.5, ScreenRelativeY gDefaultButtonScreenRelativeY
0.6)}` (main pc 120-159). Input handlers: Accept f27, Back f28, Misc (Square) f29 = **empty**, Up f30, Down f31,
L1 f32 = page up, R1 f33 = page down. No Left/Right handler: Left/Right fall to the template default (button links
of the command stack; the stack has no left/right links, so nothing) [D]. No `music`: the Singleplayer music keeps
playing [V]. Background: the static `iface_instant_action` 512x512, its top 448 rows full screen (re-bf2-shell §3.4).

### 1.2 Layout - every widget (PS2, English) [V code, D numbers, M frame check]
Numbers below use the PS2 values: safe area 460.8 x 403.2 from (25.6, 22.4) (re-bf2-shell §3.5), `gamefont_tiny` line
height 15, `gamefont_small` 18, `gamefont_large` 22 (`analysis/re-hud.md` §1.1). Builder: main.f34.
`ScreenRelativeY 0` puts y = 0 at the **safe area's top** (22.4 units): with that origin the predicted Planet window
(centre y 175.5 + 22.4 = 197.9) matches the frame's window body 109..287 units (`ref_frames/ia_planet_list.png`) [M];
x is from the screen centre 256.

| widget | code | value (units) |
|---|---|---|
| list font | f34 pc 2-12 | `gamefont_tiny` on PS2 (PC `gamefont_small`, others `gamefont_medium`) |
| three column widths | f34 pc 13-15 | left R3 = safeW x 0.375 = 172.8; centre gap R4 = safeW x 0.25 = 115.2; right R5 = 172.8 |
| list row height | f34 pc 28-63 | English: Map / Mode / Playlist rows `font h + 2` = **17**; extra window height R8 = 30 (other languages: `2 x (h+2)`, 44). Era rows `2 x (h + 4)` = **38** (two-line era names) |
| rows shown | f34 pc 64-96 | `clamp(floor((safeH - 240) / 17), 4, 16)` = **9** for Map, Mode, Playlist; Era `0.5 x 9` = 4.5 -> 4 row widgets (the create loop runs 1..4.5) [D] |
| list widths | f34 pc 16-27; listmanager main.f0 pc 16-27 | `layout.width` = 172.8 - 35 = 137.8; on non-PC the slider is dropped and width += 30 -> **167.8** |
| Planet window | f34 pc 97-120 | `NewButtonWindow{ZPos 200, x -(R5+R4)/2 - 2 = -146 (centre x 110), y 78 + H/2 + 6 = 175.5, width 172.8, height H = 9 x 17 + 30 = 183, titleText 'ifs.missionselect.selectplanet' "Planet"}` |
| Mode window | f34 pc 121-149 | same rectangle, title "Mode" (`ifs.missionselect.selectmode`); pc 121-130 compute smaller heights for Mode / Era that are **never used** [V] |
| Era window | f34 pc 150-168 | same rectangle, title "Era" (`ifs.missionselect.selectera`) |
| Playlist window | f34 pc 169-190 | x +(R5+R4)/2 - 2 = **+142** (centre x 398), same y / size, title "Playlist" (`ifs.missionselect.playlist`) |
| info box | f34 pc 191-240 | `NewButtonWindow{x -2, y 78 + 183 + 38 + 10 = 309 (screen 331.4), width safeW 460.8, height 76}`, Text1 = `NewIFText{x -220.4 (= -safeW/2 + 10), y -34, left / top, gamefont_tiny, textw 440.8, texth 76, colour gSelectedTextColor (138,160,244), random start delay 0..0.5 s, no bar}` [M: frame box 314..393 px] |
| command stack | main pc 87; f34 pc 241-244 | `AddVerticalButtons(buttons, {yTop -130, HardWidthMax 80, font 'gamefont_tiny', bAllSquareButtons 1, buttons launch / add / remove / options})`: rows 15 + 6 = 21 tall every 25 [M: LAUNCH / ADD / REMOVE / OPTIONS centres at y 172 / 199 / 226 / 252 px = pitch 25.2 units], no title, square caps |
| preview rect | f34 pc 245-256 | `movieX/Y/W/H` = 132, 188, 380, 310 on PS2 - **set but never used**: no movie name exists for this screen (§6) |
| window skin | `ifelem_buttonwindow` main.f7 | 9-slice `border_3_pieces` with 16-unit corners (main.f0); title bar = `NewIFText` in `gMenuButtonFont` (`gamefont_large`), row 22 + 6 = 28 tall, 4 above the window, over `bf2_buttons_topleft / title_center / topright`, bar width = window width - 26, title text `gTitleTextColor` (246,235,20), `flashy=1, startdelay=0`, shifted +8 in x (pc 107-163) |
| scroll arrows | `ifelem_listmanager` main.f0 pc 16-100 | non-PC: two `listbox_topbot_more` halves (x -32..2 and the mirrored 32..-1, 16 tall) 6 above the first row and below the last; shown when the list scrolls [M: the down arrow under KAMINO]. Down moves the cursor to the 9th visible row, then **scrolls one row per press** (no margin); L1 / R1 move **8 rows** (0 -> 8 -> 16 -> 24) [M menuref `ia_list_scroll`, `ia_list_page`] |
| focus box | listmanager main.f0 pc 137-154 | `listbox_cursor` 9-slice, list width x (row h + 10) = 167.8 x 27 |

List rows (CreateFn, all `nocreatebackground=1`, `startdelay = random x 0.5`) [V]:
- Map row (f0): one text, x 4, left/top, width - 10.
- Mode row (f1): text width - icon - 10 and a mode icon at the right edge, square of side `0.75 x row h` = 12.75
  (non-English: half that), ZPos 240 (f1 pc 10-18, pc 61).
- Era row (f2): text (two lines fit in 38) and icon1 at the right, side `0.375 x 38` = 14.25, drawn grey (128,128,128)
  then tinted with the row colour (f2 pc 57; populate f6 pc 23-33).
- Playlist row (f3): map name, then the mode icon at the right edge and the era icon1 left of it (3 + 5 units apart),
  both 12.75 square; the era icon grey 128 (f3 pc 64-89) [M: two small icons right of "HOTH" in `ia_playlist_launch.png`].
- Row colours: the focused row (138,160,244) a 0.6, the others (240,240,240) a 0.6, from `ListManager_fnFillContents`
  (listmanager main.f3 pc 66 / 79) through each PopulateFn's `IFObj_fnSetColor` + `IFObj_fnSetAlpha` (f4-f7).
- `gamefont_tiny` draws capitals only on the frames ("CORUSCANT" for "Coruscant") [M].

### 1.3 States and what each press does [V]
`ifs_ms_state = {map 0, mode 1, era 2, command 3, playlist 4}` (main pc 0-6). Enter (f24) always lands in
**command** (pc 31-36); the first Enter also expands the map list (`missionlist_ExpandMaplist(bForMP)`), reads
`iMaxMissions = ScriptCB_GetMaxMissionQueue()` (= 128: `FUN_00360890` returns 0x80, B2 357683-357686) and clears
the playlist to the single row "Remove All" (`gPickedMapList = {{Map='_remove_all_', bIsRemoveAll=1}}`, f13;
`mapname.name._remove_all_` = "Remove All") (f24 pc 15-30). Every Enter also calls
`ifs_instant_options_fnResetDefaults` (pc 4-5; = `set_defaults` + `push_prefs`, which re-read the engine's stored
values, so the session options survive the round trip, §2.3).

| state | visible windows (f12) | Up / Down | L1 / R1 | Cross (f27) | Triangle (f28) | info box (f9) |
|---|---|---|---|---|---|---|
| command | Planet (no cursor), Playlist (no cursor), stack focused | stack moves, skipping dimmed (f30 pc 13-20) | nothing (no list focused) | LAUNCH -> `fnLaunch`; ADD -> state map; REMOVE -> state playlist, the cursor on its **last** row [M menuref `ia_remove`]; OPTIONS -> push `ifs_instant_options_overview` (pc 34-66) | `ScriptCB_PopScreen()` -> Singleplayer (pc 30-33) | `ifs.missionselect.buttons.help.<tag>`: "Launch the current playlist." / "Add a map to the playlist." / "Remove a map from the playlist." / "Modify the session options." |
| map | Planet (cursor), Playlist | list move (no wrap) | page up / down (8 rows [M menuref `ia_list_page`]) | -> state mode (pc 4-9) | -> command (pc 3-8) | `missionlist_GetLocalizedMapDescr` of the row: "<Planet>: <site>" + description [M: "CORUSCANT: JEDI TEMPLE / A sprawling planetary metropolis..."] |
| mode | Mode (cursor) replaces Planet, Playlist | list move | page | -> state era (pc 13-18) | -> map (pc 12-17) | the mode's `descstr` (`modename.description.<m>`) |
| era | Era (cursor) replaces Mode, Playlist | list move | page | **adds** (`fnAddMap`) -> state command, focus LAUNCH (pc 22-33) | -> mode (pc 21-26) | the era's `showstr` only ("Galactic Civil War") [M `ia_era.png`]; pc 40-61 build "<map> <mode>" and discard it [V] |
| playlist | Planet (no cursor), Playlist (cursor) | list move | page | on "Remove All": clear the list + `errorSound`; on a map: delete that row, `SelectedIdx = min(idx, n)`; then -> command, focus ADD if only "Remove All" is left, else LAUNCH (pc 70-119) | -> command (pc 38-42) | "" on "Remove All", else the entry's map name (pc 91-114) |

After every press: the sound (§4), `fnUpdateScreen` and `ScriptCB_SetSpectatorMode(0, nil)` (f27 pc 120-129) [V].
Changing column fades the four list windows 1 -> 0 / 0 -> 1 over `gShellAnimTime` 0.4 s (f8) [V; see §5].

Command buttons' dim rule (f11) [V]: in states map / mode / era / playlist **all four** are `bDimmed`; in command,
LAUNCH is dimmed while the playlist has fewer than 2 rows (only "Remove All") and the other three are live. REMOVE is
live even with an empty playlist (Cross then enters the playlist on "Remove All"). `SetCurButton` keeps the last
command focused while a list is focused (f11 pc 42-56), so that button keeps its focus pulse [D; `ia_planet_list.png`
shows ADD blue while the Planet cursor is on CORUSCANT [M]]; the list cursor does **not** pulse beside it (only one
pulse on screen: ADD at 0.734 s, the cursor box's edge pixels identical over 5 s) [M menuref §9.1.9,
`measure/ia_list_pulse.csv`]. First focus with an
empty playlist: the first non-dimmed = ADD [D][M `ia_missionselect.png`].

Help bar (template + f10): bottom-left Triangle "Back", bottom-right Cross **"Select"** (`bAcceptIsSelect`; f10 sets
`common.select` again and moves the icon) [V][M]. Square, L1, R1 have no help entry.

### 1.4 Adding, removing, launching [V]
- `fnAddMap` (f19) -> `fnAddMap1(map, mode, era)` (f18): "All Maps" loops every non-wildcard map ->
  `fnAddMap2` (f17): "All Eras" loops every non-wildcard era the map has (`map[era.key]`) -> `fnAddMap3` (f16):
  "All Modes" loops every non-wildcard mode with `map[mode.key .. '_' .. era.subst]` -> `fnAddMap4` (f15).
- `fnAddMap4` (f15): if the list (with its "Remove All" row) already holds more than `iMaxMissions` = 128 rows, hide
  the lists, open `Popup_Ok` "Selected maplist is full." (`ifs.missionselect.listfull`; done -> show lists, f14) and
  stop; else append `{Map = string.format(mapluafile, era.subst, mode.subst)` (e.g. `hot1%s_%s` -> `hot1g_con`),
  `Side 1, SideChar era.tag, Team1 era.Team1Name, Team2 era.Team2Name}` and select it (pc 37-68). Duplicates are
  allowed [D: no check].
- `fnDeleteMap(i)` (f20): rebuilds the list without row i.
- `fnLaunch` (f21): with 2 rows or more: drop row 1 ("Remove All"), `ScriptCB_SetMissionNames(gPickedMapList,
  bRandomOrder)`, `fnDone()` (pc 3-16) -> `ifs_instant_fnSelectDone` pushes `ifs_instant_side` when `SelectedMap`
  is set (`ifs_instant_top` main.f0) -> stops the movie, `ScriptCB_SetTeamNames`, `ScriptCB_EnterMission()`
  (`ifs_instant_side` main.f0). With only "Remove All": `errorSound` (pc 18-20) - unreachable by pad: LAUNCH is
  dimmed and skipped while the playlist is empty [M menuref "Instant Action"].
- `bRandomOrder` (play the list in order or shuffled) is toggled only by the Host group's "Map Playlist" row
  (`ifs_instant_options` main.f6 pc 321-325), which is hidden in single player: **SP Instant Action always plays in
  order** [D].

## 2. Session Options

### 2.1 `ifs_instant_options_overview` ("Session Options") [V][M `session_options.png`]
- Stack `{xWidth 305, width 305, font gMenuButtonFont (gamefont_large), title 'ifs.instantoptions.title' "Session
  Options"}`: Global, Host, Hero, Conquest, CTF, Hunt, Assault (main pc 30), centred (no left-justify), container at
  0.5 / 0.6; `movieBackground 'shell_sub_left'`, `bAcceptIsSelect` (help: Back / Select) (main pc 40-48).
- Enter: Host is hidden unless `ifs_missionselect.bForMP` (main.f0 pc 9) -> SP shows **Global, Hero, Conquest, CTF,
  Hunt, Assault** [M]. There is no Elimination button although the tags file has an Elimination group [V].
- Cross: `ifs_instant_options_SetOptionGroup(tag)`, accept sound, push `ifs_instant_options` (main.f1). Triangle:
  pop (main.f2).

### 2.2 `ifs_instant_options` (one page per group) [V][M `options_global.png`, `options_conquest.png`]
- `music 'shell_soundtrack'`, no `movieBackground` (keeps `shell_sub_left`) (main pc 150).
- Builder main.f22 (PS2 = PC path: pc 0-2 jump to pc 3 either way): rows `gamefont_small`, row height
  `2 x (18 + 2)` = **40**, rows shown `floor((403.2 - 65 - 120) / 40)` = **5**; window `NewButtonWindow{ZPos 200,
  centred, y 230/2 + 30 = 145 (screen 167.4), width safeW x 0.85 = 391.7, height 5 x 40 + 30 = 230, title
  'common.mp.options'}`, re-titled per group (main.f8 pc 48: "Global Options", "Hero Options", "Conquest Options",
  "CTF Options", "Hunt Options", "Assault Options"); list width 391.7 - 40 (+30 on PS2) (pc 52-60). Row text x 10,
  left, `valign vcenter`, width - 20 (main.f1). [M: the frame's window 70..570 px wide = 400 units.]
- Info box: container at the safe area's left, y 230 + 65/2 + 5 = 267.5 (screen 289.9); background `white_rect`
  grey (128,128,128) a 0.5 from x 4 to safeW - 8, 65 tall; text x 10, y 65/4 - 10, width safeW - 20, font
  `r0.listboxfont` (never set on this screen -> nil) (main.f22 pc 61-106); the PS2 draws it in the rows' face and
  size, `gamefont_small` (mixed case, white, lines 19.6 units apart), not `gamefont_tiny` [M menuref
  `sheets/session_options.png`, §9.1.4]. Text =
  `ifs.mp.createopts.helptext.<tag>` of the focused row (main.f7).
- Help bar: Back only (Accept hidden, main.f9 pc 34); the Square "Set Password" line is MP-only (pc 20-38).
- Pad: Up/Down = list (main.f12/f13); Left / Right = -1 / +1 (f14 / f15), L1 / R1 = -10 / +10 (f16 / f17), each
  then `shell_select_change` and refill; Cross does nothing; Triangle = `push_prefs` then pop (f10).
- Rows are text with inline arrows: every value key reads "Label: {OptionL} %s {OptionR}"; `{OptionL}` /
  `{OptionR}` are drawn as the COMMON textures `optionl` / `optionr` (the small blue chevrons on the frames) [D: the
  tokens are in the text table, the two textures are in COMMON.LVL and baked; the engine's token substitution is not
  read].

### 2.3 Every option (SP; the `adjust` is main.f6, the text main.f2, the groups `ifs_instant_options_tags`) [V]
Step = the Left/Right step; L1/R1 multiply it by 10. Clamp(v, lo, hi) (main.f5) - no wrap.

| group | tag | text (English) | field | step | range | default | what it changes |
|---|---|---|---|---|---|---|---|
| Global | numbots | "AI Units Per Team: < n >" | `iNumBots` | 1 | 0 .. `iMaxBots` = **0 .. 16** [M menuref `so_global`] | 16 [M] (`DAT_0066ca88`, runtime) | AI per team; a change also copies the value into con / ctf / eli / ass numbots (pc 12-29) |
| Global | aidifficulty | "AI Difficulty Level: < Normal >" | `iDifficulty` | toggle `5 - x` | 2 "Normal" <-> 3 "Elite" (1 "Easy" shows but cannot be reached) | profile's difficulty (profile+0x3c, `ScriptCB_GetNetGameDefaults` B2 44185-44198, clamp 1..3) [M Normal] | the bots' skill level (`analysis/re-ai-difficulty.md`); **the port replaces this row's three levels with its four tiers, §7** |
| Hero | heroes_onoff | "Heroes: < On/Off >" | `bHeroesEnabled` | toggle | | **Off** [M menuref `so_hero`] | heroes in the battle; Off hides the other hero rows (main.f8 pc 3-14) |
| Hero | hero_unlock_1 | "Unlock Hero: < Points / Timer >" | `HeroPrefs.iHeroUnlock` | toggle 3 <-> 5 | only Points (3) and Timer (5) reachable; texts exist for 1-6 | Points [M menuref `so_hero`] | the unlock rule |
| Hero | hero_unlock_2 | "First to < n > points" / "< n > seconds" (by rule) | `iHeroUnlockVal` | 1 | 1 .. 120 | 10 [M menuref `so_hero_ranges`] | the unlock threshold |
| Hero | hero_assign | "Assign To: < Best Player / Worst Player / Random Player >" | `iHeroPlayer` | 1 -> 4 -> 7 (Right), 7 -> 4 -> 1 (Left) | 1, 4, 7 (no wrap) | 1 Best [M menuref `so_hero_ranges`] | who gets the hero |
| Hero | hero_respawn_val | "Timer: < Always / n seconds / Never >" | `iHeroRespawnVal` | 1 | 0 ("Always") .. 120 s, then -1 "Never" (121 wraps to -1); Left from Never -> 120 [M] | **90 s** [M menuref `so_hero_ranges`] | hero respawn delay |
| (forced) | - | - | `iHeroTeam` | - | - | set to 3 "Both teams (separately)" on every push (main.f20 pc 4) | - |
| Conquest | con_numbots | "AI Units Per Team: < n >" | `iCONNumBots` | 1 | 0 .. `iMaxBots` | 16 [M] | AI per team in conquest |
| Conquest | con_mult | "Reinforcements: < n% >" | `iCONMult` | 10 | 10 .. 500 % | 100 % (0x5fa7f0) | starting reinforcements scale (`re-time-limit.md` §5) |
| Conquest | con_timer | "Timer: < Off / n >" | `iCONTimer` | 5 | 0 "Off" or 5 .. 60 min | 0 Off (0x5fa8c8) | conquest time limit (`re-time-limit.md`) |
| CTF | ctf_numbots | "AI Units Per Team: < n >" | `iCTFNumBots` | 1 | 0 .. `iMaxBots` | 16 [M menuref `so_ctf_hunt_ass`] | AI per team (1- and 2-flag) |
| CTF | ctf_score | "CTF Score: < n >" | `iCTFScore` | 1 | 1 .. 15 | 5 (0x5fa850) | captures to win |
| CTF | ctf_timer | "Timer: < Off / n >" | `iCTFTimer` | 5 | 0 .. 60 min | 0 Off (0x5fa8b0) | CTF time limit |
| Hunt | hun_timer | "Timer: < n >" | `iHUNTimer` | 5 | **5** .. 60 min | 5 (0x5fa868) | hunt time limit |
| Hunt | hun_score | "Score Limit: < n >" | `iHUNTScoreLimit` | 10 | 10 .. 1000 | 50 (0x5fa880) | hunt score to win |
| Assault | ass_numbots | "AI Units Per Team: < n >" | `iASSNumBots` | 1 | 0 .. `iMaxBots` | 16 [M menuref `so_ctf_hunt_ass`] | AI per team (space) |
| Assault | ass_score | "Score Limit: < n >" | `iASSScoreLimit` | 10 | 50 .. 1000 | 180 (0x5fa898) | assault points to win |

- The texts' keys: `ifs.mp.createopts.numbots`, `.difficulty`, `.heroes`; `ifs.mp.create.opt.reinforcements`,
  `.timer` (+ `.off` "Off"), `.ctf`, `.scorelimit`; `ifs.mp.heroopts.herounlock1..6`, `.numpoints`, `.numseconds`,
  `.heroplayer1/4/7`, `.respawntimer` (+ `common.always`, `common.never`, `.second`, `.seconds`); help lines
  `ifs.mp.createopts.helptext.<tag>`, e.g. numbots "Set the number of AI units to fight on each team for all modes.",
  con_numbots "Set the number of AI units per team for all Conquest matches in your session." [V, resolved with
  `locl_bf2.tsv`].
- Defaults: `set_defaults` (main.f19) reads `ScriptCB_GetNetGameDefaults()` / `ScriptCB_GetNetHeroDefaults()`
  (engine) and fills only missing keys (iDMMult 100, iCONMult 100, iCONTimer 60, iCTFScore 5, iCTFTimer 60, iHUNTimer
  5, iHUNTScoreLimit 50, iASSScoreLimit 180, per-mode numbots = iNumBots); the engine always returns every key, so the
  values in the table above are the engine's (`re-time-limit.md` §2 table; the "60" fallbacks never fire) [V].
- `iMaxBots` = `DAT_0060056c` = 16 in the ELF image, raised to `DAT_00600574` = 25 on the non-network branch
  (`ScriptCB_GetNetGameDefaults` B2 44184-44236) [V]; on the PS2 single-player Session Options the cap is **16**
  (Right x3 and R1 at 16 stay 16; minimum 0) [M menuref `so_global`, §9.1.1]. A Global change is copied into
  Conquest, CTF and Assault (Global 10 -> all three read 10) [M menuref `so_global`].
- The values survive leaving to the Instant Action screen and coming back (Global 0 / Elite, Conquest 150 % kept)
  [M menuref `so_persist`].
- Per-mode numbots come from tables at 0x5fa808 / 0x5fa818 / 0x5fa828 / 0x5fa838 indexed by the rule set, clamped
  to `iMaxBots` when read (B2 44270-44300) [V]. They hold -1 in the ELF image (`elfrd` 0x5fa808 = 0xffffffff), and
  `FUN_0019bfc8` replaces every negative entry with `DAT_006004fc` = **16** (B2 43305-43324) [V] - so the default is
  16, as the Conquest page shows [M]; who calls `FUN_0019bfc8` was not followed [H].
- Elimination (`eli_numbots`, `eli_mult` 10..500 step 10, `eli_timer` 0..60 step 5) and Host (`playlistorder`,
  `players`, `warmup`, `vote`, `teamdmg`, `shownames`, `autoassign`, `startcnt`, `voicemode`) are not reachable in SP.

## 3. The rest of the menu tree

Scope: every PS2 screen reachable from the title / main menu except Instant Action (`ifs_missionselect`) and its
Session Options. Sources: the raw Lua 5.0 listings `engine/artifacts/lead_tmp/re/bshl/dis/<lvl>_<script>.txt`
(cited `script proto pc`; the lifts in `bshl/lift/` print the same pcs), English via the BF2 text table
(`lead_tmp/re/evmsg/locl_bf2.tsv`; key hash = FNV-1a over `byte | 0x20`, so `_` hashes as 0x7f), frames in
`bshl/ref_frames/`. Tags: [V] read, [D] derived, [H] hypothesis, [M] seen on the named frame.
Shared rules from `analysis/re-bf2-shell.md` §3 are not repeated: the vertical stack (row = font height + 6, pitch + 4,
bar = widest label + ~134 units, title bar blue / rows grey / last row rounded), `ScreenRelativeY` 0.6 default, the
dimmed style (label grey 110, focus skips it), the help bar (Triangle "Back" bottom-left, Cross "Accept"/"Select"
bottom-right; "Select" when `bAcceptIsSelect`), left-justified stacks' bar edge at x 34.4 units.

Pad names on the PS2 (help-bar icons): Accept = Cross `btna`, Back = Triangle `btnb`, `Input_Misc` = Square
`btnmisc`, `Input_Misc2` = Circle `btnmisc2` [M profile_name.png: "Backspace" with the Square icon, "Symbols" with
the Circle icon]. `Input_LTrigger`/`RTrigger` = L1/R1 [H: the names; the controller map's `ltrig1` uses `btn_R1`].
A screen that sets no `movieBackground` keeps the movie already playing [M difficulty.png, profile_name.png: the
movie still runs behind both].

### Flow (PS2, single player, one pad)

| from | press | to |
|---|---|---|
| `ifs_start` | Start | memory-card check (`ifs_saveop` `InitialMemcardCheck`) -> `ifs_login` (`ifs_start` main.f8 pc 4-17, main.f11 pc 2-5) |
| `ifs_login` (no profiles) | Cross on Create | `ifs_vkeyboard` "Enter profile name:" (main.f41 pc 29-63) |
| `ifs_vkeyboard` | Cross on Accept | a non-empty unique name -> `ScriptCB_AddProfile`, `ifs_difficulty` (ifs_login main.f7 pc 4-52); a **one-letter name is accepted** [M menuref `kb_accept2`]; empty -> Popup_Ok "Please enter at least one character" (main.f5) [M menuref `kb_accept`] |
| `ifs_difficulty` | Cross | `ScriptCB_SetDifficulty(tag, true)`, pops back to `ifs_login`, which saves (main.f6 pc 1-7; main.f32 pc 66-70) |
| save prompt | Save / Continue Without Saving | -> `ifs_main` (`ifs_login_Done` -> `ifs_start_OnLoginDone`, `ifs_start` main.f10 pc 2) |
| `ifs_login` (profiles exist) | Cross on a name | load it (`ifs_saveop` `LoadProfile`) -> `ifs_main` |
| `ifs_main` | Singleplayer / Multiplayer / Splitscreen / Options | `ifs_sp` / `ifs_mp` / `ifs_split_profile` / `ifs_opt_top` (main.f19 pc 13-139) |
| `ifs_main` | Triangle | profile dirty -> save popup; else log out and pop to `ifs_login` (main.f18, main.f7) |

### ifs_start (Press START) - covered in re-bf2-shell §2 row 3; additions

| item | value |
|---|---|
| widgets | `logo` image 512x256 at y -38 (main pc 20); prompt `ifs.start.press_start` "Press START Button To Begin", `gamefont_large`, textw 460, y -210 from the bottom, `valign bottom`, startdelay 0.4 (main pc 34); invite text (Xbox only, hidden, main.f0 pc 15) [V] |
| help bar | none (`bNohelptext`, main pc 71) [V] |
| pads | Start only (main.f8); Triangle does nothing (main.f1 pc 0); Cross, directions do nothing (main.f3-f7) [V] |
| sounds | `enterSound ''` (silent enter); Start plays `acceptSound` (main.f8 pc 23) [V] |
| no-pad text | `ifs.start.nocontroller` is never shown on the PS2: with 0 pads the PS2 branch keeps "Press START" (main.f2 pc 17-35) [V] |

### ifs_login (Pick a Profile)

Script `shell/ifs_login`. Movie `shell_main` (main pc 128). `bAcceptIsSelect`; enter / exit sounds silent [V].

| widget | value |
|---|---|
| list window | `NewButtonWindow` title `ifs.profile.title` "Pick a Profile", centred, y -40, width 260 + 35 = 295, height 5 rows x (font h + 6 = 28) + 30 = 170 (main.f44 pc 9-35); rows `gListboxItemFont` (`gamefont_large`), text x 10, left-aligned, textw 220 (main.f0 pc 26); 5 shown, slider (main pc 18) [V][M profile_pick.png] |
| button | one stack button `new` "Create" (`ifs.profile.create`), width 260, `gMenuButtonFont` (main pc 143), placed just below the window (main.f44 pc 45; main.f32 pc 38-49) [V][M profile_pick.png: "Create" in a one-row bar under the list] |
| help bar | Back, Select; plus "Delete" (`ifs.profile.delete`) with Square, bottom-right, y -40 (main pc 101), shown only while a name is highlighted (main.f2 pc 25-31) [V] |
| info box | `LoginInfoWindow` 280x75 at the bottom, `gamefont_medium` (main.f44 pc 59-76): Xbox Live only, hidden on the PS2 (main.f32 pc 133-137) [V] |

| pad | does |
|---|---|
| Up/Down | moves between the names and Create; Create sits below the last name (main.f35, f37) [V] |
| L1/R1 | page up/down in the list (main.f36, f38) [V] |
| Cross | Create -> list full (`ifs.Profile.listfull`, Popup_Ok_Large) or the keyboard (MaxLen 10 on the PS2, `MaxWidth` = safe width x 0.32; main.f41 pc 32-63); a name -> load it (pc 65-75) [V] |
| Square | on a name: Popup_YesNo "Are you sure you want to delete this profile?", focus No (main.f42 pc 13-30) -> `ifs_saveop` `DeleteProfile` (main.f16) [V] |
| Triangle | pops back to the title (main.f34 pc 6-9) [V] |
| Enter | first loads the card's file list (`ifs_saveop` `LoadFileList`, main.f10); `ScriptCB_GetLoginList` fills the list (main.f9 pc 2) [V] |

### ifs_vkeyboard (name entry)

Script `shell/ifs_vkeyboard`. `nologo`, no movie of its own (main pc 102) [V].

| item | value |
|---|---|
| title | `gamefont_large`, top-centre (y 0), textw 460, text set by the caller: "Enter profile name:" (`ifs.vkeyboard.title_login`) (main pc 48; ifs_login main.f41 pc 60) [V][M profile_name.png] |
| window | `NewButtonWindow` centred at `ScreenRelativeY` 0.46, width = safe width (460.8), height = safe height x 0.6 (241.9), y = -0.05 x height (main.f28 pc 0-20) [V] |
| key grid | 4 rows x 13 columns (`vkeyboard_specs` main pc 6); column pitch XDiff = (460.8 - 32)/13 = 32.98; row pitch YDiff = min(241.9 x 0.58 / 4, XDiff x 1.2) = 35.08; first key at x = -(460.8 - 32)/2 = -214.4, y = -1.5 x YDiff = -52.6 (main.f28 pc 58-80) [V][D numbers]; keys `gamefont_medium`, textw XDiff x 1.1 (pc 117) [V]; characters from `ScriptCB_GetVKeyboardCharacter(row x 13 + col + offset)` (main.f12 pc 86): page 0 = A-M / N-Z / a-m / n-z [M profile_name.png]; Circle toggles offset 0 <-> 52 (symbols page; main.f15) [V] |
| name box | `NewButtonWindow` width safe w x 0.65 + 10, height YDiff x 1.4, y = -0.4 h + 0.3 YDiff (pc 124-139), resized on Enter to (safe w - 20) (main.f22 pc 93); text `gamefont_medium`, centred (pc 159) [V][M "Player 1" in light blue] |
| cursor | `NewButtonWindow` 0.9 XDiff x 0.9 YDiff behind the focused key (pc 163-177) [V] |
| bottom row | horizontal buttons "Cancel" / "Backspace" / "Accept" (`common.cancel`, `ifs.vkeyboard.backspace`, `common.accept`), `gamefont_medium`, each safe w x 0.32 wide, y = 0.4 x height (pc 180-211); row index 4 [V][M] |
| help bar | Square "Backspace" bottom-left y -40 (main pc 57); Circle "Symbols"/"Letters" bottom-right y -40 (main pc 68, main.f15); Back; Accept relabelled "Select" (main.f28 pc 10) [V][M] |
| start state | focus on Accept (column floor(13 x 0.85) = 11, row 4), text = the default name (`ScriptCB_GetUniqueLoginName`, "Player 1"); the first key press clears it (`bFirstAction`, main.f22 pc 19-74, main.f12 pc 61-66) [V][M] |
| pads | Cross types the key / runs the bottom button (main.f12); Start = done (main.f13); Square = backspace (main.f14); Circle = symbols (main.f15); Up/Down/Left/Right wrap the cursor; L1/R1 move too (main.f16-f21) [V]; sounds `shell_keyboard_key`, `shell_keyboard_backspace` [V] |

### ifs_difficulty (Select Difficulty)

Script `shell/ifs_difficulty`. `nologo`, no movie of its own (main pc 1-4) [V]; the previous movie keeps running [M difficulty.png].

| item | value |
|---|---|
| stack | centred, `ScreenRelativeY` 0.6, `width` 460, ySpacing 5, `gMenuButtonFont`; title `ifs.difficulty.title` "Select Difficulty"; buttons `2` "Normal" (`ifs.difficulty.medium`), `3` "Elite" (`ifs.difficulty.hard`) (main pc 21-37) [V][M difficulty.png] |
| focus | the current `ScriptCB_GetDifficulty()` (main.f0 pc 13-18) [V] |
| Cross | `ScriptCB_SetDifficulty(tag, true)` then the caller's `fnDone` (main.f2 pc 12-14) [V]; the click heard is **exit** (the screen pops back to the profile code), then the save popup [M menuref `kb_accept2`] |
| Triangle | default pop; when reached from profile creation it logs out (`LogoutOnCancel`, main.f1) [V] |
| help | Back, Select [M] |

### Save prompt (Popup_LoadSave2, from ifs_saveop)

Built by `common/popup_loadsave2`, filled by `common/ifs_saveop` main.f0 [V].

| item | value |
|---|---|
| popup | `NewPopup` centred, width 460, height 280, `ButtonHeightHint` 95, `bNoThinPopup` (main pc 68); text `gamefont_small`, textw 440, y2 -80 (pc 51) [V] |
| text | `ifs.loadsave_ps2.save20` "Would you like to save %s to the memory card (PS2) in MEMORY CARD slot 1?" with the profile name (ifs_saveop main.f0 pc 82) [V][M save_popup.png] |
| buttons | stack (C, B, A), `gPopupButtonFont` (`gamefont_small`), width 300, yTop -10 (popup_loadsave2 main pc 18); C hidden, B "Continue Without Saving" (`.continuenosave`), A "Save" (`.save`) (ifs_saveop main.f0 pc 21-62); focus A (pc 67) [V][M] |
| other uses | main-menu Back with a changed profile: A "Save and Exit", B "Exit Without Saving", C "Cancel", text `save24` "The current profile has changed. Exiting now will lose these changes.  Would you like to save first?" (ifs_main main.f7 pc 19-73) [V] |

### Popups (common)

`gPopup_fnSetTitle_Internal` (`common/popups_common` main.f0) sizes every popup to its text [V]:
- text box width starts at `width x 0.75 - 32`; it widens to `width - 32` when `bNoThinPopup`, or the text is taller
  than 0.45 x `height`, or the box is under 1.5 x `gButtonWidthPad` (150) (pc 5-53);
- window = (text width + `gPopupHeightPad` 32) x (text height + `gPopupWidthPad` 32 + (`ButtonHeightHint` or 50))
  (pc 104-117); the frame centre y = text y2 + H/2 - 16; buttons at y = y2 + text height + 14 (pc 118-135).
- Fonts: text `gPopupTextFont`, buttons `gPopupButtonFont`, both `gamefont_small` on the PS2 (interface_util) [V].

| popup | script / pc | size | text | buttons |
|---|---|---|---|---|
| Popup_Ok | popup_ok main pc 20-35 | 260 x 180, hint 45 | textw 220, y2 -80 | "OK" (`OkButton_layout`: width 300, popups_common main pc 11); Cross and Triangle both close |
| Popup_Ok_Large | popup_ok_large main pc 14-29 | 400 x 240 | `gamefont_medium`, textw 370, y2 -110 | "OK" |
| Popup_YesNo | popup_yesno main pc 24-43 | 300 x 180, hint 75 | textw 260, y2 -80 | vertical "Yes" / "No" (width 300, popups_common pc 39); Up/Down move |
| Popup_YesNo_Large | popup_yesno_large main pc 16-31 | 440 x 220, hint 70 | textw 410, y2 -100 | "Yes" / "No" |
| Popup_Ask_Historical | ifs_sp_campaign main pc 63-99 | 400 x 240, hint 70 | textw 370, y2 -110 | "Play" / "Skip" (`ifs.sp.play`, `ifs.sp.skip`) |
| Popup_LoadSave2 | above | 460 x 280, hint 95 | textw 440 | up to 3 |

### ifs_main (Main Menu) - covered in re-bf2-shell §2 row 5; additions

| item | value |
|---|---|
| layout | `width` 205, xSpacing 6, ySpacing 5, container y +10 at 0.6, `logo` at y -38 (main pc 37, 75, 87) [V] |
| list order | sp "Singleplayer", mp "Multiplayer", split "Splitscreen", tutorials "Tutorials", opts "Options", back2 "Profile Management", quit "Exit To Windows", quitdemo; on the PS2 tutorials, back2, quit, quitdemo hidden (main.f15 pc 149-205) [V] |
| Multiplayer | no network adaptor -> Popup_Ok_Large "No Network Adaptor detected. You will not be able to access the online features of this title." (main.f19 pc 48-65); else `ifs_mp` (pc 73-78) [V] |
| Triangle | profile dirty -> the 3-button save popup (above), **no sound**, focus Save and Exit [M menuref `main_back`]; else `ScriptCB_Logout()` and back to `ifs_login` (main.f7 pc 87-92) [V] |
| help | Back, Select (`bAcceptIsSelect`; `bNohelptext_backPC` hides Back on PC only) (main pc 107) [V] |

### ifs_sp (Singleplayer) - covered in re-bf2-shell §2 row 6; additions

| item | value |
|---|---|
| layout | left-justified, `width` 300, xSpacing 10, ySpacing 5, title "Singleplayer" ("Splitscreen" in split mode, main.f6 pc 48-61) (main pc 27) [V][M singleplayer.png] |
| buttons | training "Training", spacetraining "Space Overview", riseempire "Rise of the Empire", meta "Galactic Conquest", ia "Instant Action" [V] |
| hidden / dimmed | Training hidden in splitscreen; Rise of the Empire `bDimmed = not 1` = never dimmed (main.f3 pc 0-10) [V] |
| actions | Training -> mission `geo1c_c` under `campaign` rules (main.f4 pc 17-30); Space Overview -> `ifs_spacetraining`; Rise of the Empire -> (training not done: Popup_Ask_Historical "Do you want to play the training mission first?" Play/Skip, main.f5) -> `ifs_freeform_rise_newload`; Galactic Conquest -> `ifs_sp_campaign`; Instant Action -> rules `instantaction`, `ifs_instant_top` (split: `ifs_split_map`) (main.f8) [V] |
| save | a dirty profile is saved (ifs_saveop, normal save prompt) before the next screen and on Back (main.f8 pc 59-71, main.f9) [V] |
| focus on return | coming back to Singleplayer (Triangle from Instant Action, Galactic Conquest, Rise of the Empire, or quitting a mission) focuses **Training**, not the entry left [M menuref `ia_enter_back`, `rise2`] |

### ifs_freeform_rise_newload (Rise of the Empire)

| item | value |
|---|---|
| movie / music | `shell_sub_left`, `shell_soundtrack` (main pc 37) [V] |
| stack | left-justified, centred container at `ScreenRelativeY` 0.5 (not 0.6), `width` 400, title "Rise of the Empire"; "New Game" (`ifs.meta.load.new`), "Load" (`ifs.meta.load.btnload`) (main pc 18-29) [V] |
| actions | New Game -> clears metagame / campaign / mission state -> `ifs_campaign_main` (the campaign map; out of scope); Load -> `ifs_campaign_load` mode `Load` (main.f2) [V] |
| help | Back, Select [V] |

### ifs_sp_campaign (Galactic Conquest)

| item | value |
|---|---|
| movie / music | `shell_sub_left`, `shell_soundtrack` (main pc 137) [V] |
| stack | left-justified, container at `ScreenRelativeY` 0.5, `width` 460, title "Galactic Conquest" (`ifs.sp.meta`) (main pc 48, 123) [V] |
| buttons | `1` "Birth of the Rebellion", `2` "The Confederate Uprising", `3` "Republic Sovereignty", `4` "Dark Reign of the Empire", `custom` "Versus", `campaign` "Campaign List", `load` "Load" (main pc 48) [V] |
| hidden / dimmed | Versus shown only in splitscreen; Campaign List hidden when `gFinalBuild` (engine global, [H] true on retail); 1-4 and Load never dimmed (`bDimmed` = false; main.f4 pc 0-60) [V] |
| actions | 1-4 -> `ifs_freeform_start_all/cis/rep/imp(ifs_freeform_main)` then `ifs_freeform_main` (the galaxy map) (main.f9 pc 95-140); Versus -> `ifs_freeform_customsetup` (custom campaign: Era, Layout Random/Fixed, Victory Total / Base / Turn limit / Credits; `iface_bgmeta_space` backdrop); Load -> `ifs_freeform_load`; each asks the training question first (main.f6) [V] |
| cheat | Square, Circle x3 alternating (`iCheatState` 0-6) -> `ScriptCB_SetSPProgress(2, 2)` (main.f11, f12) [V] |
| help | Back, Select [V] |

### ifs_spacetraining (Space Overview) and ifs_trailer (View Trailer)

Both full-screen movie players [V]:

| item | ifs_spacetraining | ifs_trailer |
|---|---|---|
| movie | closes the shell movie, opens `movies\train.mvs` (per language: trainfr / traingr / trainit / trainsp / trainuk), property `train` (main.f0 pc 7-29) | `movies\trail.mvs` (trailfr / trailgr), property `trail` (main.f1 pc 7-26) |
| sound | music `STOP`; stops the VO and music streams, fades `shellfx` (main.f0 pc 31-48) | same (main.f1 pc 28-45) |
| help bar | none (`bNohelptext_accept`, `bNohelptext_back`) | none |
| pads | Triangle stops the movie (main.f4); the screen pops when the movie ends (main.f2); directions do nothing | same (main.f5, main.f3) |
| exit | reopens `gMovieStream` (main.f1 pc 12) | same (main.f2) |
| port | not built (no mode behind it) -> hide in Singleplayer | not built -> dimmed in Options |

### ifs_mp (Pick Connection Type) and the PS2 network chain

| item | value |
|---|---|
| ifs_mp | movie `shell_main`, `nologo`; stack centred at 0.6, `width` 400, title "Pick Connection Type": "Internet", "LAN", (Direct Connection, Journal, Playback hidden on the PS2: main.f2 pc 134-158); LAN dimmed while no cable, checked every 0.75 s (main.f0); IP / build strings only when not `gFinalBuild` (pc 41-46) [V] |
| Internet | `gOnlineServiceStr` = GameSpy -> `ifs_mpps2_netconfig` (main.f4 pc 82-89) [V] |
| LAN | -> `ifs_mpps2_netconfig` (main.f4 pc 18-22) [V] |
| ifs_mpps2_netconfig | "Select Network Configuration" list window (width safe w x 0.98, `ScreenRelativeY` 0.4; rows 100 high: name `gamefont_medium` + 3 `gamefont_tiny` lines), backdrop `iface_bgmeta_space`, music STOP; Square "Add / Edit"; reads the memory card ("Checking memory card (PS2)...") (main pc 63-147, main.f43 pc 29) [V] |
| then | connected -> Internet: `ifs_mpps2_dnas` (DNAS logo, "Authenticating ...", first time `ifs_mpps2_eula` "SOFTWARE LICENSE AND LIMITED WARRANTY" scroll box, then `ifs_mpps2_patch`); LAN: `ifs_mp_main` (netconfig main.f19 pc 9-17; dnas main.f6 pc 23-36) [V] |
| ifs_mp_main | "Multiplayer" stack, `width` 400, backdrop `iface_bgmeta_space`: Client (quick), Join, Create, Join IP, Buddies, Downloadable Content, (recent), Personal Stats, Leaderboard, Options, Sign Out; several hidden by login state (main pc 42, main.f16 pc 110-232) [V]; which rows the PS2 shows: not captured |
| port | Multiplayer stays dimmed on the main menu (re-bf2-shell §9.4); none of this is built |

### ifs_split_profile / ifs_split_map (Splitscreen)

| item | value |
|---|---|
| reached | Main Menu -> Splitscreen (only with 2 pads; dimmed otherwise) -> `ifs_split_profile` (ifs_main main.f19 pc 105-114) [V] |
| layout | `nologo`, `bNohelptext`, movie `shell_sub_left`; title `gamefont_large` at the top (main pc 90-152); the PS2 has 2 players (`iMaxControllers` 2; Xbox 4: main.f24 pc 0-5): the safe area split into a top and a bottom half, full width, with a horizontal line `gray_rect` (192,192,255) alpha 0.8, 4 units thick, across the centre (main.f24 pc 21-28, 217) [V][D halves] |
| per half | "Press Start Button To Join" (`gamefont_medium`), a profile list (width min(0.8 x half width, 260), 4 rows of 26, `gamefont_medium` names), "Press START Button To Launch", own Back / Select helptexts with Triangle / Cross at the half's bottom corners (main.f24 pc 73-196) [V] |
| texts | "Ready", "Please Insert DUALSHOCK(R)2", "Please connect DUALSHOCK(R)2 analog controller into controller port %s" [V keys] |
| Start | when every joined player has a profile: loads them (main.f23 pc 25-104), then `ifs_split_profile_*Done` (main.f11 pc 26-53) pushes `ifs_sp` in split mode (title "Splitscreen", Training hidden; `gMovieDisabled` = 1) [V] |
| ifs_split_map | no screen of its own: pushes `ifs_missionselect` with `bShortSideSelect`, then `ScriptCB_EnterMission()` (main.f1 pc 16-21, main.f0 pc 13) [V] |
| port | Splitscreen dimmed until SSCR |

### ifs_opt_top (Options) - covered in re-bf2-shell §2; details

Script `common/ifs_opt_top`. `nologo`, movie `shell_sub_left`, `bDimBackdrop`, `bAcceptIsSelect` (main pc 69) [V][M options_top.png].

| tag | label | PS2 visible | opens |
|---|---|---|---|
| general | Game Options | yes | `ifs_opt_general` |
| pcvideo | (Video Options) | hidden (non-PC, main.f3 pc 11-15) | - |
| sound | Audio Options | yes; in game only for the pausing player (pc 68-86) | `ifs_opt_sound` |
| cveh | Controls (`ifs.controls.Vehicle_Unit`) | yes (pc 27-31) | `ifs_opt_contmode` |
| pccontrols | Control Options | hidden | - |
| online | Online Options | yes (dimmed only in the demo build, pc 88-95) | `ifs_opt_mp` |
| unlock | Special Features | always hidden (pc 34) | `ifs_unlockables` |
| trailer | View Trailer | shell only, not under GC / campaign (pc 35-50) | `ifs_trailer` |
| credits | Credits | shell only (pc 51-54) | `ifs_credits` |
| viewstats | Career Stats | shell only (pc 64-67) | `ifs_careerstats` |
| fonttest | Font Test | hidden (`gFinalBuild`) | - |

Layout: `width` 360, ySpacing 5, `gMenuButtonFont`, title "Options" (`ifs.Main.options`), left-justified in the shell
(main.f6 pc 6-13), centred in game [V]. Triangle: shell and profile dirty -> save (normal prompt) then pop; else pop
(main.f5) [V]. In the pause menu the stack holds Game Options, Audio Options, Controls, Online Options [D].

### ifs_opt_general (Game Options)

Script `common/ifs_opt_general`. `nologo`, no movie of its own, `bDimBackdrop`, help: Back only
(`bNohelptext_accept`) (main pc 87) [V].

| item | value |
|---|---|
| window | `NewButtonWindow` title "Game Options"; width = safe w x 0.85 = 391.7; in the shell left-aligned (`ScreenRelativeX` 0, x = width/2), in game centred; `ScreenRelativeY` 0.5 (main.f11 pc 73-96) [V] |
| rows | `gListboxItemFont` (`gamefont_large`); row height = 2 x (font h + 2) = 48 (the 1.3 x branch is dead code: pc 28-30 jump over pc 31-38); rows shown = min(10, floor((safe h - 140)/48)) = 5, slider (main.f11 pc 5-72) [V][D 48, 5]; text x 10, left-aligned, width - 20 (main.f0 pc 33) [V] |
| list (shell, no network) | Viewpoint, Vibration, Friendly Fire, Auto Target, Aim Assist, Sticky Reticule, Display Enemy Icons, AI Difficulty, Tool Tips, Objective Details (main pc 12; chosen main.f2 pc 1-28) [V] |
| other lists | in game no net: same minus Friendly Fire (main pc 30); net (shell or game): Viewpoint, Vibration, Tool Tips (main pc 18, 36) [V] |
| reset | Square "Reset ToolTips" helptext built (main pc 69) but hidden (main.f2 pc 43) [V] |
| store | `ScriptCB_GetGeneralOptions()` table on fill (main.f2 pc 29-31); every change -> `ScriptCB_SetGeneralOptions(t)` (main.f6 / f8 pc 103) [V]; Back pops (main.f7); profile marked on Enter and reloaded only for `_back` (unused tag) (main.f3-f5) [V] |

Every row is "Label: {OptionL} value {OptionR}" (arrows drawn around the value [H: engine replaces the tokens]).
Left (main.f6) and Right (main.f8) do the same except Tool Tips:

| tag | text (key) | field | values | Left / Right |
|---|---|---|---|---|
| persp | "Viewpoint:" (`ifs.gameopt.camera2`) | `bFirstPerson` | "1st Person" / "3rd Person" | toggle |
| rumble | "Vibration:" (`rumble2`) | `bRumble` | "Yes" / "No" | toggle |
| friendlyfire | "Friendly Fire:" (`friendlyfire2`) | `iFriendlyFire` | < 1 -> "Off", else "On" | `100 - x` (0 <-> 100) |
| autoaim | "Auto Target:" (`autoaim2`) | `bAutoAim` | "On" / "Off" | toggle |
| aimassist | "Aim Assist:" (`aimassist2`) | `bAimAssist` | "On" / "Off" | toggle |
| stickyreticule | "Sticky Reticule:" (`stickyreticule2`) | `bStickyReticule` | "On" / "Off" | toggle |
| soldierhints | "Display Enemy Icons:" (`soldierhint`) | `bSoldierHints` | "On" / "Off" | toggle |
| diff | "AI Difficulty:" (`difficulty2`) | `iDifficulty` | 1 "Easy", 2 "Normal", 3 "Elite" | `< 3 -> 3, else 2`: Normal <-> Elite only |
| ttstate | "Tool Tips:" (`ttstate2`) | `bToolTips`, `bToolTipAuto` | "On" / "Auto" / "Off" | Left `ScriptCB_PrevToolTipState()`, Right `ScriptCB_NextToolTipState()` |
| objectivedetails | "Objective Details:" (`objectivedetails`) | `bObjectiveDetails` | "On" / "Off" | toggle |
| (hero, moviesubtitles) | "Instant Action Heroes:", "Movie Subtitles:" | `bHeroes`, `bMovieSubtitles` | On / Off | handled but in no PS2 list |

Defaults: not in the scripts (`ScriptCB_GetGeneralOptions`, `ScriptCB_ResetGameOptionsToDefault` are engine). On a
fresh profile [M menuref `game_options`]: Viewpoint 3rd Person, Vibration Yes, Friendly Fire On, Auto Target On, Aim
Assist On, Sticky Reticule On, Display Enemy Icons On, AI Difficulty Normal, **Tool Tips Auto**, Objective Details On.
Tool Tips Right cycles **Auto -> Off -> On** -> Auto, Left the other way; every Left / Right plays accept.

### ifs_opt_sound (Audio Options)

Script `common/ifs_opt_sound`. `nologo`, no movie of its own, `bDimBackdrop`, help: Back + Square "Reset to
defaults" (`ifs.SoundOpt.reset`, bottom-left y -40; main.f23 pc 144) [V].

| item | value |
|---|---|
| title | "Audio Options" `gamefont_large`, top-centre, textw 460 (main.f23 pc 29) [V] |
| labels | right-justified text column ending at x = safe w x 0.4 - 8 (container `ScreenRelativeX` 0.4), `gamefont_medium` (`gamefont_tiny` in German), yTop 55, row 35, ySpacing 0 (main pc 31, 144; main.f23 pc 46-65) [V] |
| sliders | `NewHSlider` width = safe w x (0.75 - 0.4) = 161.3, height 24, textures `slider_sound` (bg) / `slider_fg` (thumb), at x = width/2 + 20 right of the label edge, one per volume row (main.f22 pc 14-64; main.f2) [V][D width] |
| values | a value column (stack `width` 60, right) showing "N%" (main.f23 pc 69-75; main.f0) [V] |
| focus | the focused slider alpha 1, the others 0.5 each frame (main.f21; main.f18 pc 38-45) [V] |

| row | label | PS2 | range / step | getter / setter |
|---|---|---|---|---|
| master | "Master Volume" | shown | 0..max, Left/Right +-1, L1/R1 +-10, clamped (main.f7/f8 pc 3-11; main.f14-f17) | `ScriptCB_GetVolumes()` -> (music, sfx, voice, battlevoice, max, master); `ScriptCB_SetVolumes(music, sfx, voice, battle, master)` (main.f7 pc 94) |
| music | "Music Volume" | shown | same | same |
| sfx | "SFX Volume" | shown | same; plays `shell_auditionSFX` on bus `soundfx` | same |
| voice | "Speech Volume" | shown | same; `shell_auditionVO` on `voiceover` | same |
| battlevoice | "Battle Speech Vol" | shown | same; `shell_auditionVO` on `battlechatter` | same |
| output | "Output Mode" | shown (main.f10 pc 19-30) | the texts "Mono" / "Headphones" / "Stereo" / "Dolby Pro Logic(R) II" exist, but the PS2 cycles only **Stereo -> Dolby Pro Logic II -> Mono -> Stereo** (Right; no Headphones; default Stereo) [M menuref `audio_options`]; Left `ScriptCB_PreviousOutputMode`, Right / Cross `ScriptCB_NextOutputMode` (main.f6 pc 1-46) | `ScriptCB_GetOutputMode()` |
| bassmang | "Bass Management" | hidden (PC only, main.f10 pc 58-61) | - | - |
| mixconfig | "Mixer Config" | hidden (main.f10 pc 31-39) | - | - |
| effects | "Effects" | hidden (pc 40-48) | - | - |
| reset | "Reset" | hidden (pc 49-57); Square does it instead | Popup_YesNo_Large "Do you want to reset sound settings to the defaults?", focus No -> `ScriptCB_ResetSoundToDefault()` (main.f20, main.f9) | - |

Value display: when max = 10 the label is value x 10 "%", else value / max x 100 "%" (main.f0 pc 42-159) [V]; the
PS2 max is [H] 10 (read `ScriptCB_GetVolumes` in RAM). Defaults [M menuref `audio_options`, `audio_reset`]:
**Master 100 %, Music 60 %, SFX 70 %, Speech 90 %, Battle Speech 100 %, Output Stereo**; step 10 %, L1 / R1 jump to
0 % / 100 %; a volume change plays its audition 0.12-0.49 s after the press, the Music row nothing. On Enter the in-game buses
(`soundfx`, `ambience`, `voiceover`, `battlechatter`) fade to 0 and the shell music interval to 0 (main.f10 pc
139-166), restored on Exit (main.f11) [V].

`ifelem_hslider` geometry (`common/ifelem_hslider`) [V]: defaults width 100, height 16 (main.f2 pc 0-9);
`thumbwidth` is always 0.1 (it reads `thumbpercentage`, never the caller's `thumbwidth`; pc 10-14); bg image ZPos 160,
thumb ZPos 159 (pc 59-75). `HSlider_MoveThumb` (main.f0): thumb spans x = -w/2 + w x pos .. + 0.1 w, full height;
the bg is stretched to -w/2 .. w/2 + 0.1 w. Alpha set on both (main.f1).

**openbf settings** (issue #128). The shared shell and pause options group game,
audio, graphics and controls settings. Controls first chooses keyboard/mouse or
gamepad. Sound Effects and Dialogue volumes scale the authored effects and
speech gains independently of music. The pause menu's Game Options holds
the session cheat controls. The FPS counter is optional and reports presented
frames with render scale in Advanced mode. Instant Action exposes its Hero
and Assault groups. Conquest hero eligibility extends to bots only when AI
heroes is enabled.

### ifs_opt_contmode (Controls -> Select Control Type)

Script `common/ifs_opt_controller_mode` [V]:

| item | value |
|---|---|
| stack | `width` 350, ySpacing 5, `gMenuButtonFont`, title "Select Control Type" (`ifs.controls.controlmodes`); left-justified in the shell (main pc 44-51); `bDimBackdrop`, silent enter / exit, Select help |
| buttons | "Infantry Controls", "Vehicle Controls", "Starfighter Controls", "Jedi Controls", "Turret Controls" (main pc 24) |
| Cross | `ScriptCB_SetControlMode(0..4)` then `ifs_opt_controller` (main.f0 pc 19-66) |

### ifs_opt_controller (the pad layout page)

Script `common/ifs_opt_controller_vehunit` (+ `ifs_opt_controller_common` for the PS2 button map,
`controller_presets` for the presets) [V]:

| item | value |
|---|---|
| title | the mode's name ("Infantry Controls" ...), `gamefont_medium`, top-centre (main pc 458; main.f13 pc 51-88) |
| picture | texture `controller` at local (-24, -214)..(232, 42) from screen centre (main pc 514) |
| buttons | 22 `Platform_btn_map` entries (`ifs_opt_controller_common` main pc 376): L2, L1, D-pad up/left/down/right, left stick, Select, Start, right stick, Square, Cross, Circle, Triangle, R1, R2, Flip Y, L3, R3, Y sensitivity, Choose Preset, X sensitivity; each with a hot-spot (x, y, w, h, highlight texture `btn_R1`, `Dpad_up`, `btn_stick`, `btn_circle` ...), Left/Right/Up/Down neighbours `conL/R/U/D`, and a callout line from (lx2, ly2) to a `gamefont_tiny` label at (lx1, ly1) (main.f10 pc 42-121) |
| label text | "{icon} function", e.g. `{btna} Jump` (`gButtonNames`, `gButtonFunctionStrings`; main.f27 pc 66-81); Select = "Map", Start = "Pause Menu" fixed (pc 122-136) |
| cursor | `NewButtonWindow` 120 x 42 with `listbox_cursor` texture (main pc 501; main.f13 pc 12) |
| start focus | "Choose Preset:" (index 21) (main.f13 pc 7-15) |
| Cross on a button | opens a function list (`ifs_controller_listbox_layout`: 5 rows of 35, width 180 + 35, `gamefont_tiny`) beside the pad, filtered per mode (`contents_functype_*`), Cross assigns (`ScriptCB_SetFunctionIdForButtonId` / `...ForAnalogId`) (main.f15 pc 48-243) |
| Choose Preset | preset list (5 rows of 40, width 250 + 35): "Obi-Wan", "Anakin", "Custom" (`ifs.controls.presets.A/B/custom`, one table per mode `gControllerPresets0..4`); leaving Custom asks Popup_YesNo "Custom config will be lost. Proceed?" (main.f15 pc 245-301; main.f25) |
| sensitivity | "X-Axis Sensitivity: < n >", "Y-Axis Sensitivity: < n >" (`gamefont_small`, x -270), Left/Right step 1, range 1..10, wraps (main.f21 pc 25-80; main.f26) -> `ScriptCB_SetControlScale(0/1, n)` |
| Flip Y | "Flip Y-Axis: " + state, Left/Right toggle `ScriptCB_SetYAxisFlip` (main.f21 pc 83-112) |
| vibration | not here: it is Game Options "Vibration" |
| pads | Up/Down/Left/Right follow `conU/D/L/R` (main.f21 pc 5-23); L1/R1 page the open list (main.f18, f20); Triangle closes the list, else pops (main.f16) |
| port | build later; the layout tables above are complete enough to draw it |

### ifs_opt_mp (Online Options)

Script `common/ifs_opt_mp`. `nologo`, `bDimBackdrop`, Back only (main pc 57) [V]. A list window titled
"Online Options" like Game Options (rows `gamefont_small`, main.f0 pc 31). PS2 rows (`ifs_opt_mp_listtags` main pc 8;
LAN drops `players`): GameSpy ID automatic login ("Ask every time" / "Always" / "Never"), Hosting Bandwidth
(`iPlayersSupported` 0..5: DSL/Cable ... T1, T1 x 2), Network performance icon (`iOnlineIcon` 0/1), Headset Record
Volume (0..10), Headset Playback Volume (0..10), Enable voice (On/Off) (main.f3, main.f6 pc 42-128); stored with
`ScriptCB_GetOnlineOpts` (main.f5 pc 11) [V]. Port: dimmed (no online).

### ifs_careerstats (Career Stats)

Script `common/ifs_careerstats` [V]: `nologo`, full-screen image `statsscreens_bg` (ZPos 250), title bar "Career
Stats" (`gamefont_medium`, `ScreenRelativeY` 0.017, title colour, bar pieces `bf2_buttons_topleft/title_center/topright`,
bg_width 460) (main pc 108-119); stat boxes 2 columns x 4 rows (rows 20 + 2, width 250, `gamefont_tiny`, label white
left 90 %, value yellow right 35 %); medal boxes 5 + 4 rows (19 + 3) with name, level ("Green" / "Veteran" / "Elite"
/ "Legendary") and count; rank title ("Private" ... "General", "As a Private you can command 1 soldier...") (main pc
21-57; main.f1-f3). Data: `ScriptCB_GetCareerPersonalStats`, `ScriptCB_GetCareerMedals`,
`ScriptCB_GetCareerMedalLevel`. Port: show only if the port keeps such stats; else dimmed.

### ifs_credits (Credits)

Script `shell/ifs_credits` [V]: no movie of its own (`movieBackground` nil, main pc 2010), help: Back only
(`bNohelptext_accept`). A crawl: line container tilted `rotX` 40 (the Star Wars slant), `UseSafezone` 0, ZPos 210
(main pc 1981); 593 entries in `CreditLines` (main pc 1955), each {str, font, colour}: defaults `gamefont_small`
white, ySpacing 2, min line height 20; headings `gamefont_large` / `gamefont_medium` with ColorB 0 = yellow
(main pc 9, 1955); lines flagged `bNoPS2` are skipped (two: Bink, libpng; main.f1). Scroll speed 40 units/s
(`ySpeed`, main.f2 pc 8); text x = (screen w x aspect - safe w x 0.62)/2, width safe w x 0.62 (main.f10 pc 20); the
screen pops after the last line (main.f2 pc 26-29). Port: the line table can be lifted as data straight from
`dis/shell_ifs_credits.txt` main pc 0-1955.

### ifs_unlockables (Special Features)

Not reachable on the PS2: Options hides it always (`ifs_opt_top` main.f3 pc 34) [V]. (10 BF1-era stills / art
movies `nabbonus` ... in `movies\bonus.mvs`, Xbox only.) Not built.

### ifs_pausemenu (in game)

Script `common/ifs_pausemenu` [V]. `nologo`, no movie, `bDimBackdrop`, `bAcceptIsSelect`; stack centred at
`ScreenRelativeY` 0.5 (split viewports: `ifs_pausemenu1..3`, y -30), `width` 260, ySpacing 5, title "Game Menu"
(`game.pause.title`) (main pc 42-147).

| tag | label | PS2 single player |
|---|---|---|
| resume | Resume Game | shown -> `ScriptCB_Unpause` (main.f9 pc 10-14) |
| freecam | Free Camera / Chase Camera | hidden (`gFinalBuild`, main.f7 pc 108-110) |
| lobby | Player List | net game only (pc 77-82) |
| opts | Options | shown -> `ifs_opt_top` (main.f9 pc 108-119); hidden when the game is over |
| console | Fake Console | hidden (`gFinalBuild`) |
| suicide | Respawn | shown -> Popup_YesNo_Large "Do you really wish to kill your current player?" focus No |
| friends | Buddies | hidden unless logged in |
| recent | (recent players) | hidden unless Xbox Live |
| restart | Restart Mission | hidden in GC (`metagame`) and net games (pc 84-95) -> "Do you really want to restart this battle?" |
| quit | Quit | shown -> `ScriptCB_SetQuitPlayer(1)`, Popup_YesNo_Large focus No: "Do you really want to quit this battle?"; GC: "...This will be counted as a loss."; host: "Leaving the game now will end this session. Are you sure you want to leave?" (main.f9 pc 17-61) |
| exit | Exit To Windows | hidden (PC only, pc 97-106) |

Triangle = resume (`exitSound`, `ScriptCB_Unpause`; main.f14) [V]. In single player the PS2 shows the host text
"Leaving the game now will end this session. Are you sure you want to leave?" [M menuref `pause_menu`].

The original 4:3 Hero Assault pause capture shows a dim scene, with the
HUD gone and a centered, translucent six-bar stack. Measured in the shell's
512 × 448 frame, it is about 320 units wide and centered at (258, 221).
The blue title and gray buttons use the shell's segmented art. The opening
sweeps in each bar from the left and the selected row pulses.

The port uses this presentation for ground and space battles (#128), with
the same text keys and original row order. Quit shows the host text and
confirmations open on No. Cheats and the test class picker follow Options
when available. Options stays after the round ends and combines the
port's settings in one page. The Options, Cheats and confirmation pages
use compact BF2-styled bars with the existing actions. The Back control
resumes from any page (playtest A3). A page's own Back row returns to the
Game Menu. Partial bakes without the menu art can use the fallback panel.

### Hidden on the PS2 (do not draw)

`ifs_tutorials` (main-menu row hidden, `ifs_main` main.f15 pc 178), `ifs_unlockables`, `ifs_fonttest`, Video /
Control Options (PC), Profile Management / Exit To Windows (PC), Direct Connection / Journal / Playback (ifs_mp),
Free Camera / Fake Console (pause) [V].

### For the port: shown, live or dimmed

| screen | row | port |
|---|---|---|
| Singleplayer | Training, Space Overview | hidden (no mode) - or dimmed to keep the PS2 list intact |
| Singleplayer | Rise of the Empire, Galactic Conquest | dimmed until M5 |
| Options | Online Options, View Trailer, Credits, Career Stats | dimmed until built (Credits is cheap: data + crawl) |
| Game Options | rows the port does not honour (Auto Target, Aim Assist, Sticky Reticule, Tool Tips, Objective Details) | dimmed rows (label grey 110, focus skips) |
| Audio Options | Output Mode | dimmed (the port is stereo) |
| Controls | Select Control Type -> pad page | dimmed until a rebinding card |
| pause | Restart Mission | live in a battle launched from the shell (the playlist row loads again); hidden in a `--map` launch |

## 4. Menu sounds

Tags: [V] read, [D] derived, [H] hypothesis. Lua cites are `file proto pc` from the raw listings in
`engine/artifacts/lead_tmp/re/bshl/dis/` (`common_*` = COMMON.LVL, `shell_*` = SHELL.LVL). The lift's pcs are the
raw CALL pcs (checked on `shell_ifs_missionselect main.f27` and `shell_ifs_movietrans main.f6`), but the lift drops
register set-up, so every branch below was read in the raw listing.

| Event | Sound | Where the data is | Baked? |
|---|---|---|---|
| Focus move in a button stack (up/down/left/right) | `shell_select_change` | CORE.LVL bank `global` | sample yes (`core_bf2/sounds`); nothing in the port plays it |
| Move in a listbox (up/down, L1/R1 page, scroll) when the cursor actually moves | `shell_select_change` | CORE | same |
| Left/right on an Instant Action option (also L1/R1 = ±10) | `shell_select_change`, always | CORE | same |
| Left/right on an Options > General row; mix / output / effects rows of Audio | `shell_menu_accept` | CORE | same |
| Volume sliders (master, sfx / voice, battle voice) | the `shell_auditionSFX` / `shell_auditionVO` stream line, not a click | SHELL.LVL stream `shell_vo` | yes (`common_bf2/shell/sounds/streams/b082ecb1`) |
| Accept (Cross) | the screen's `acceptSound` = `shell_menu_accept` | CORE | sample yes; not played |
| Back (Triangle) | the screen you land on plays its `exitSound` = `shell_menu_exit` | CORE | sample yes; not played |
| Refused action | `errorSound` = `shell_menu_error` (same sample as cancel) | CORE | sample yes; not played |
| Popup "no" / cancel | `shell_menu_cancel` | CORE | sample yes; not played |
| Forward screen enter | `enterSound` = '' (nothing) | - | - |
| Screen transition whoosh | `shell_transition1..10` - **never fires in BF2** (no screen has a `movieIntro`) | SHELL.LVL bank `shell` | WAVs baked, dead data |
| Shell music | `shell_soundtrack` (4 takes of `shell_music`): a random take each time, ~10 s apart [M menuref `coldboot_music`] | SHELL.LVL stream `shell_music` | yes (raw ADPCM `.bin`) |

### 4.1 What plays when

All 277 sound calls in the 134 scripts were listed (30 `ScriptCB_SndPlaySound` + 248 `ifelm_shellscreen_fnPlaySound`
globals minus the one definition = 277). [V] Distinct names played: `shell_select_change`, `shell_menu_accept`,
`shell_menu_exit`, `shell_menu_cancel`, `shell_menu_error`, `shell_menu_enter`, `shell_menu_ok`, `shell_keyboard_key`,
`shell_keyboard_backspace`, `shell_transition<N>`, plus the streams `shell_auditionSFX` / `shell_auditionVO` through
`ScriptCB_ShellPlayDelayedStream`.

**The template** (`common_ifelem_shellscreen`):
- `ifelm_shellscreen_fnPlaySound(name)` = `ScriptCB_SndPlaySound(name)` when `name` is set (main.f2 pc 0-4). [V]
- Defaults filled in by `NewIFShellScreen` (main.f30 pc 14-42): `enterSound` '', `exitSound` `shell_menu_exit`,
  `acceptSound` `shell_menu_accept`, `cancelSound` `shell_menu_cancel`, `errorSound` `shell_menu_error`,
  `selectSound` `shell_select_change`; `movieIntro` is forced nil (pc 43). [V]
- Enter(this, bFwd) (main.f10): bFwd -> `enterSound` (pc 44), else (re-entered after a pop) `exitSound` (pc 48);
  then the screen's `music` (pc 50-60, 'STOP' = stop). [V]
- Input_Back (main.f12) is just `ScriptCB_PopScreen` (pc 1): no sound of its own. So **Back = the exit click of
  the screen underneath** on its re-Enter. [D]
- Up/Down (main.f22/f23, f26/f27 for pad 2): a focused listbox first (`gShellScreen_fnDefaultInputUp/Down` main.f20
  pc 14 / f21 pc 14 -> `ListManager_fnNavUp/Down`), else `FollowButtonLink(..., 'link_up'|'link_down')` (f22 pc 13,
  f23 pc 13); Left/Right the same with `link_left/right` (f24 pc 7, f25 pc 7). [V]
- PC mouse-only: a listbox click plays `shell_select_change` (main.f19 pc 24). [V]

**Button stacks** (`common_ifelem_button main.f3` = `FollowButtonLink`): plays `shell_select_change` when it finds
a target button (pc 36-40), skipping hidden or dimmed ones. Links wrap last <-> first, so every press in a stack
clicks, including the wrap. Focus never lands on a dimmed button, so a pad can't press one and there is no
"dimmed press" sound. [V][D]

**Listboxes** (`common_ifelem_listmanager`): NavUp f6 pc 27 only if `SelectedIdx > 1`; NavDown f7 pc 27 only if
not on the last row; PageUp f8 pc 27 / PageDown f9 pc 34 only if it moves; ScrollUp f10 pc 11 / ScrollDown f11 pc 20
only if the window moves. No wrap. **At the top or bottom of a list, a press is silent.** [V]

**Accept per screen** - each screen's Input_Accept plays its own `acceptSound` (the template has no accept sound):
- `ifs_start` Start: accept (main.f8 pc 23); its enterSound is '' (main pc 71). [V]
- `ifs_main` Cross: accept when a button is focused (main.f19 pc 7-11); its Exit plays `exitSound` when left
  backwards (main.f16 pc 0-4). [V]
- `ifs_sp` (main.f8 pc 9), `ifs_sp_era` (main.f1 pc 8), `ifs_sp_campaign` (main.f9 pc 20), `ifs_opt_top`
  (main.f4 pc 8), `ifs_instant_options_overview` (main.f1 pc 5), `ifs_mp` (main.f4 pc 14/48/70),
  `ifs_tutorials` (main.f2 pc 2): accept. [V]
- Refusals that play `errorSound`: `ifs_sp_briefing` (main.f12 pc 66), `ifs_unlockables` locked item (main.f15
  pc 17; otherwise accept pc 21), `ifs_split_profile` (main.f21 pc 19), vkeyboard bad key / not acceptable
  (`shell_ifs_vkeyboard` main.f12 pc 102, f27 pc 47). [V]

**Instant Action** (`shell_ifs_missionselect`; states map -> mode -> era -> command, and playlist):
- Cross (main.f27): R1 := `acceptSound` at pc 0, played at pc 120-122 in every state: pick map, mode, era (era also
  adds to the playlist, pc 24-26), Add, Remove, Options, Launch. Exception: Cross on the playlist's **"Remove All"**
  row (`gPickedMapList[1]`, set at main.f13 pc 6) plays `errorSound` instead (pc 79). [V]
- Launch with an empty playlist (only the "Remove All" row): `ifs_missionselect_fnLaunch` plays `errorSound`
  (main.f21 pc 3-20), then f27 still plays accept (pc 122) [D] - **unreachable by pad**: LAUNCH is dimmed and
  skipped while the playlist is empty [M menuref "Instant Action"].
- Triangle (main.f28): `exitSound` in every state (pc 45), including stepping era -> mode -> map -> command. From
  the command state it also pops (pc 33): **one** exit click is heard (its gain 2.2 against 3.5 for a plain exit;
  two stacked would be ~double) [M menuref `ia_back_chain`, §9.3.4].
- Up/Down: the focused list's NavUp/Down (main.f30 pc 6) -> click only if it moved; in the command state the
  button stack (pc 17). L1/R1: PageUp/PageDown (main.f32 pc 6, f33 pc 6). [V] Held Up/Down repeat from 0.61-0.65 s,
  then every 0.167 s (5 updates), each with a click, as on the main menu [M menuref `ia_cmd_updown`, `main_buttons`].
- Square (Input_Misc, main.f29): empty. [V]
- Screen sets no `music`: the previous screen's music continues. [V]

**Instant Action options** (`shell_ifs_instant_options`): Left/Right = value ±1 (main.f14/f15), L1/R1 = ±10
(main.f16/f17), each then `shell_select_change` at pc 14 **with no check that the value changed** (clamped ends
still click). Up/Down = listbox (main.f12 pc 6). Back saves prefs and pops (main.f10). Sets `music`
`shell_soundtrack` (main pc 150). [V]

**Options**: General Left/Right (`common_ifs_opt_general` main.f6 / f8 pc 113) plays **accept** for every row but
`ttreset` and `back`. Audio (`common_ifs_opt_sound`): volume rows click nothing, they audition a stream line -
master `shell_auditionSFX` on bus Root, sfx on soundfx, voice `shell_auditionVO` on voiceover, battle voice on
battlechatter (main.f7 pc 104/115/126/137; same in f8 Right); music row: nothing but the music; mixconfig / output /
effects rows: accept (f7 pc 57/66/75). Cross: accept (main.f12 pc 179-215). `ifs_opt_mp` adjust: accept (main.f6
pc 2). Controller-mode and vehunit screens set enter/exit '' (main pc 41, pc 659). [V]

**Transitions** (`shell_ifs_movietrans`): `ifs_movietrans_PushScreen` (main.f3) only routes through the
whoosh screen when the target has a `movieIntro` (pc 2, 21); the whoosh is `shell_transition<N>`, N cycling 1..10
(pc 54-66), played once the intro is playing (main.f6 pc 6-15). Otherwise it fades bus `shellmusic` to 1 over 1 s,
sets the target's music and pushes (main.f2 pc 4-10). Every BF2 shell screen's `movieIntro` is nil (68 SETTABLEs
of `K'movieIntro'`, all `KNone` or LOADNIL; template main.f30 pc 43), so **the whoosh never plays in BF2**. [V][D]

**Popups**: yes/no Cross = accept either answer (`common_popup_yesno` main.f1 pc 9); large yes/no Triangle =
cancel (`common_popup_yesno_large` main.f0 pc 3); OK = accept (`common_popup_ok` main.f1 pc 23). [M menuref: Cross
on **No** plays **cancel** (pause Respawn / Quit, audio reset); "No Network Adaptor" and "Selected maplist is full."
closed by **Triangle play accept**; the delete-profile Yes/No ignores Triangle (`pause_menu`, `audio_reset`,
`main_mp`, `ia_listfull`, `profile_delete`).] load/save:
accept / cancel (`shell_popup_loadsave` main.f2 pc 5 / 34); error popup OK plays `shell_menu_ok`
(`common_error_popup` main.f1 pc 18) - **no LVL defines `shell_menu_ok`** (absent from all 41 parsed `snd_` JSONs),
so it is silent [D].

**Pause menu** (`common_ifs_pausemenu`, in battle): open = accept (main.f7 pc 210); Cross = accept (main.f9 pc 8;
pc 143 a literal `shell_menu_accept`); Triangle = exit (main.f14 pc 2; sub-pages f19, f26, f33 pc 2); quit / exit /
suicide / restart popups: yes = accept, no = cancel (main.f3-f6 pc 19/25, f5 pc 14/24). Moves = the stack / listbox
click. [V]

**Stats screens (end of battle)**: `ifs_teamstats` Enter = exit (main.f9 pc 12), Cross = `shell_menu_enter`
(main.f11 pc 21), Left/Right = select (main.f18/f19 pc 7); personal / career / award the same pattern. [V]

**Keyboard** (`shell_ifs_vkeyboard`): cursor = `selectSound` (main.f7 pc 106/110), letter = `shell_keyboard_key`
(main.f12 pc 74), delete = `shell_keyboard_backspace` (main.f12 pc 30, f14 pc 13). [V] Circle (the symbols page) also
plays `shell_keyboard_key`; Cross on Cancel plays cancel [M menuref `keyboard`].

### 4.2 Where the data lives

- **The clicks are NOT in SHELL.LVL**: they are in `extracted/bf2/lvl/DATA/_LVL_PS2/CORE.LVL` (always resident, so
  the pause menu has them too): `snd_` script + SampleBank `global` (0x1DFF06AE, data @0x800). Parsed with
  `engine/artifacts/lead_tmp/s12/re/sound_scratch/snd_parse.py` / `streams.py` (outputs `bf2_CORE.snd.json`,
  `bf2_CORE.streams.json`). All props: 2-D, bus `shellfx` (gain 0.4, parent root), priority 0.5, reverbgain 0. [V]

| Property | Sample (id) | Rate Hz | Bytes | Length s | Pitch | Gain | As heard [D] |
|---|---|---|---|---|---|---|---|
| `shell_select_change` | B21A7DEA | 16363 | 592 | 0.063 | 1.5 | 1.0 | 0.042 s; BF2 adds playinterval 0.1 |
| `shell_menu_accept` | B75D27CD | 11060 | 2704 | 0.428 | 1.1 | 1.0 | 0.389 s |
| `shell_menu_enter` | inherits accept | | | | | | same as accept |
| `shell_menu_exit` | EBAA708C | 16074 | 2352 | 0.256 | 0.8 | 1.0 | 0.320 s |
| `shell_menu_cancel` | 1C2ECDEE `ui_cancel` | 16040 | 2720 | 0.297 | 1.0 | 1.0 | 0.297 s |
| `shell_menu_error` | 1C2ECDEE `ui_cancel` | 16040 | 2720 | 0.297 | 1.0 | 1.0 | same as cancel |
| `shell_keyboard_key` | C0D52579 | 16248 | 896 | 0.097 | 1.5 | 1.0 | 0.065 s |
| `shell_keyboard_backspace` | C0D52579 | 16248 | 896 | 0.097 | 1.0 | 1.0 | 0.097 s |

  (Field hashes cracked here: 4F55EEEC `pan`, 35119EB8 `pitchdev`, F60B6465 `gaindev`, 6C408CFE `reverbgain`,
  1A8B16B5 `mode3d`, 4D5AF670 `cycleplayback`.)
- **`extracted/bf2/sound/DATA/_LVL_PS2/SOUND/SHELL.LVL`** (44 MB; parse in `lead_tmp/re/bf2menus/snd/SHELL.LVL.streams.json`,
  props in `bf2_SHELL.snd.json`; 305 props, 260 of them on `shell_vo`): [V]
  - stream `shell_vo` (B082ECB1, mono, 181 segments): `shell_auditionVO` (5 segs), `shell_auditionSFX` (1 seg), the
    mission briefings (`*_missionbriefing_*`) and ~240 unnamed VO lines.
  - stream `shell_music` (F6885C99, stereo 44.1 kHz, 8 segments, 27.4 MB).
  - stream D289DDFC (stereo, 8 segs: six 26.0 s loops + `explosion` 2 takes): name not cracked; used by six
    unnamed props + `explosion` (Galactic Conquest, not menus) [H].
  - SampleBank `shell` (11E1FC01, 20 mono samples): the L/R halves of `shell_transition1..10` (`soundlayered`, each
    `_left` pan -1 pitch 0.99, `_right` pan +1, template `shell_transition_template` bus shellfx pitchdev 0.1):
    lengths 3.76, 3.25, 3.81, 2.29, 1.78, 1.73, 3.60, 2.85, 3.73, 2.62 s (22-44 kHz).
  - **None of the click names is defined in SHELL.LVL** (no `shell_menu_*` / `shell_select_change` record). [V]

### 4.3 What the port bakes (engine/assets_baked)

- **Clicks: samples baked, not played.** `engine/assets_baked/core_bf2/sounds/` holds all 7 global-bank WAVs
  (`1dff06ae/b21a7dea.wav`, `b75d27cd`, `ebaa708c`, `1c2ecdee`, `c0d52579`, `b62dc1e9`, `d6f2e828`); `names.ron`
  names `shell_select_change`, `shell_menu_accept/exit/cancel/error/enter`, `shell_keyboard_key/backspace`; and
  `sounds.ron` holds `name_hash: 1296888771` (= `shell_select_change`: pitch 1.5, sample 2988080618) - 1 hit.
  `core_bf1/sounds/` is the same minus d6f2e828. [V] `playinterval` is not a field in the baked prop. [V]
- **Code that plays them: none.** `grep -rl --include=*.rs` under `engine/crates` (target/ excluded):
  `shell_select_change` 0, `shell_menu_exit` 0, `shell_menu_cancel` 0, `shell_menu_error` 0, `shell_transition` 0;
  control `shell_menu_accept` 4 files (only a bake test, a game test, a doc comment in `game/src/sounds.rs` and a
  comment in `game/src/main/windowed.rs:220` - which wrongly says that property "is not in SHELL.LVL - open"; it is
  in CORE.LVL, already baked), control `shell_soundtrack` 4 files. [V]
- The front end's sound library (`game/src/main/windowed/front.rs:147` ->
  `SoundLib::load(common_bf2/shell)`) loads only the shell bank: `SoundLib::load` (game/src/sounds.rs:736-758) picks
  up `core_<game>` only from `<map_dir>/manifest.ron` + `<parent>/core_<game>`, and `common_bf2/shell/` has no
  manifest.ron (its parent is `common_bf2`, not `assets_baked`), so **the front has no CORE bank and no `bf2` bus
  table**. Wiring the clicks needs the core bank loaded there too. [D]
- `common_bf2/shell/sounds/` (43 MB): the 20 `shell_transition` WAVs (`11e1fc01/`), 305 props incl.
  `shell_soundtrack` (1483632872) and `shell_transition1` (3697171064), `streams/b082ecb1/` 178 OGGs (shell VO,
  auditions, briefings), `streams/music/shell_68f000.bin` (27,426,816 B = `shell_music` raw ADPCM) and
  `shell_20b7800.bin` (D289DDFC), `streams.ron` (both music streams `baked: false` = kept as ADPCM). [V]
- Note `analysis/re-bf2-shell.md` §8 ("`shell_soundtrack` occurs nowhere in assets_baked") is out of date: the BSHLA
  shell bake (`crates/bake/src/shell.rs`) now exists under `common_bf2/shell/`. [V]

### 4.4 Music

- `shell_soundtrack` = `soundstreamproperties` on `shell_music`, bus `shellMusic` (= `shellmusic`, CORE gain 0.6),
  gain 1, looping 0, cycleplayback 0, **4 takes**: 0B495011 51.4 s, 9A38249A 51.4 s, 80C24D5E 64.4 s, D01C9F63
  56.6 s. Also on the stream: `shell_galacticconquest` 0A6C449B 52.7 s, `shell_droidinvasion` 0A6C449B (same take),
  `shell_clonewars` C27A5DAA 65.7 s, `shell_imperialmarch` 9A38249A, `shell_clash` BAE7943C 68.3 s (cycleplayback
  1); 8657D84F (133.9 s) is on no SHELL prop. [V] The PS2 picks **a random take each time, repeats allowed** (A, B, C,
  C, C, B, C over 7.5 min of one boot; the 64.4 s take never came up), each start 9.9-10.4 s after the previous
  take's length (10.6-11.0 s of heard silence with its quiet tail), the first with the Press START screen, never
  restarted by a screen change [M menuref `coldboot_music`, §9.3.9].
- Screens that set music (template Enter main.f10 pc 50-60): `shell_soundtrack` - `ifs_start` (main pc 71),
  `ifs_main` (pc 107), `ifs_sp` (pc 61), `ifs_sp_campaign` (pc 137), `ifs_sp_briefing` (pc 115),
  `ifs_instant_options` (pc 150), `ifs_tutorials` (pc 68), `ifs_freeform_pickscenario` (pc 47),
  `ifs_freeform_rise_newload` (pc 37), `ifs_unlockables` restores it after a viewer; 'STOP' - trailer, spacetraining,
  campaign battle intro, mpps2_netconfig, tutorial viewer (pc 108); Galactic Conquest sets `metagame_menu_music`
  (`shell_ifs_freeform_main`, defined in GAL.LVL). Screens without `music` (missionselect, options, sp_era) keep
  what plays. Sound options: `ScriptCB_SetShellMusicInterval(0)` on Enter, (10) on Exit. [V]
- Port: baked (raw ADPCM); `game/src/main/windowed/front.rs` plays a random take each time with the 10 s gap
  (`ui::shell_feel::ShellPlaylist`), at the take's gain x bus x the profile's music level.

### 4.5 BF1

BF1 scripts are Lua 4.0 (the 5.0 disassembler refuses them), so this is from strings only [V strings, D order]:
same template fields and the same defaults (`common/ifelem_shellscreen.luac`: enterSound/exitSound
`shell_menu_exit`/acceptSound `shell_menu_accept`/cancelSound `shell_menu_cancel`/errorSound `shell_menu_error`/
selectSound `shell_select_change`); `interface_util.luac` (BF1's FollowButtonLink) and `ifelem_listmanager.luac` play
`shell_select_change`; BF1 CORE.LVL defines the same 8 click props on the same 6 samples (BF1's `shell_select_change`
has no playinterval). BF1's shell music has the same names (`shell_soundtrack`, ...); BF1 `ifs_movietrans` has the
same `shell_transition` + whoosh code and BF1 screens do set `movieIntro` (e.g. `ifs_sp.luac` -> `ifs_sp_intro`), so
BF1 probably does play the whooshes [H]. One click set serves both games.

## 5. The focus highlight

Sources: Lua listings `engine/artifacts/lead_tmp/re/bshl/dis/<file>.txt` (cited `file proto pc`), ELF decomp
`analysis/SWBFRNT2.ELF-decomp.c` (cited `B2 <line>` / VA). Tags: [V] read, [D] derived, [H] hypothesis.
Hashes are FNV-1a of the lower-cased name (checked: `headerbuttonmid` = 0x83245ab6, `cursor` = 0xe336320f,
`normal`/`selected` = 0xe68b9c52/0x4e0a1774).

### Answer

| element | what fades | curve | range | speed | when |
|---|---|---|---|---|---|
| Vertical menu button (main menu, Singleplayer, Options, popups), flat button, square command buttons (`bAllSquareButtons`, Instant Action LAUNCH/ADD/REMOVE/OPTIONS) | the focused row's **IFObj alpha**, which scales **both the label and its bar** (the bar is the label's own flashy-text background) | linear triangle (ping-pong, clamp at ends) | alpha **0.3 .. 1.0** | **2.0 alpha/s** in 30 Hz steps: 11 updates each way, **0.734 s period** [M menuref `measure/main_idle_pulse.csv`: peaks 0.017, 0.751, 1.485, 2.219 s] | continuous every update while focused [V][M] |
| ...same row, colour | focused: label (138,160,244), bar tint = the object colour (white); unfocused: label (240,240,240), bar tint **(150,150,150)**, alpha fixed **0.6**; dimmed label 110 grey | step on focus change, no fade | | | once, on focus change [V][D] |
| Listbox (Instant Action map / mode / era / playlist lists, Instant Options rows) | the separate **`listbox_cursor`** 9-slice box behind the focused row | **none on the PS2**: the box is solid (its edge pixels identical over 5 s) [M menuref `measure/ia_list_pulse.csv`]; the script's 0.5 .. 1.0 at 1.0 /s ping-pong (`ListManager_fnHilight`) is not seen | | | [V script][M screen] |
| ...listbox rows' text | focused row (138,160,244) a 0.6, other rows (240,240,240) a 0.6 | step on refill | | | on each move [V] |
| Sound-option sliders | slider bg + thumb alpha | none (static) | focused 1.0, others 0.5 | | set every frame [V] |
| "Flashy" text (every IFText unless `flashy=0`) | a one-shot **entrance**: bar wipes out, a `cursor` square sweeps across, text grows from its centre line, then text fades in | 5 linear phases | | ELF table 0.1 / 0.05 / 0.05 / 0.15 / 0.15 s [V]; **on screen 2 / 1 / 3 / 2 / 2 updates = 0.07 / 0.03 / 0.1 / 0.07 / 0.07 s** (Singleplayer title: wipe 1.617-1.684, square 1.684, sweep 1.717-1.817, pop 1.817-1.884, solid ~1.95; last row solid 0.82 s after the press) [M menuref `measure/flashy_sp_entrance_strip.png`] | once per screen Enter [V][M] |
| start delay | menu rows: **k x 0.1 s** (k = 1.. over visible rows; title 0); list rows: **random 0 .. 0.5 s** each | | | | [V] |

Not a sine, not a colour fade: the "alive" look is a linear alpha ping-pong on the whole focused row (bar + text) for
buttons; lists have none on the PS2 (the cursor box is solid [M menuref]).

### Detail: buttons

- The pulse. `common_ifelem_roundbutton` main.f6 `RoundIFButtonLabel_fnHilight(this, on, fDt)` pc 2-32 [V]:
  `gButton_CurSizeAdd += fDt * gButton_CurDir`; above 1.0 -> clamp 1.0, dir = -|dir| (pc 8-17); below 0.3 -> clamp
  0.3, dir = +|dir| (pc 20-28); then `IFObj_fnSetAlpha(label, gButton_CurSizeAdd)` (pc 29-32). Start values
  `gButton_CurSizeAdd` 0.5, `gButton_CurDir` 2.0 (main pc 12-15). The BF1 PC source in
  `community_mods/gametoast_lua/.../Common/Scripts/ifelem_roundbutton.lua` lines 97-110 is the same code [V].
  `RoundIFButtonSkin_fnHilight` (main.f7) is empty: the skin does nothing [V].
- Who drives it. `common_ifelem_shellscreen` main.f6 (`gIFShellScreenTemplate_fnUpdate(this, fDt)`) pc 3-12 calls
  `IFButton_fnHilight(gCurHiliteButton, 1.0, fDt)` every frame; pc 13-42 do the same for splitscreen viewports 2-4,
  first restoring the saved phase so all viewports show one phase advanced once per frame [V].
  `IFButton_fnHilight` (`common_ifelem_button` main.f2 pc 0-24) defaults fDt to 1/30 and forwards to the label [V].
  [D] fDt is in seconds (the 1/30 default), so a full cycle would be 0.7 s; at the shell's 30 updates a second each
  ramp overshoots its end by one update (0.7 / 0.0667 = 10.5 -> 11), so the period is **22 updates = 0.734 s**
  [M menuref §9.4.1, §9.4.6].
- One global phase. The phase is one global for all buttons; focus moves (`SetCurButton`, `button` main.f4,
  `FollowButtonLink` main.f3 pc 38-48) never reset it, so the newly focused row picks up the current phase. On screen
  Enter / Reactivate the template calls `IFButton_fnSelect(cur, 1)` then `IFButton_fnHilight(cur, 1.0, 0.0)`
  (`shellscreen` main.f10 pc 31-39, main.f9 pc 13-21): the alpha jumps to the current phase at once [V]. Leave
  (main.f11 pc 3-7) calls Hilight with on = nil, which does nothing; the row keeps its last alpha until it is deselected [V].
- Focus change = a step, no fade. `RoundIFButtonLabel_fnSelect` (`roundbutton` main.f3) [V]:
  on: text colour `gSelectedTextColor` via `IFFlashyText_fnSetTextColor` (pc 5-13), alpha `gSelectedTextAlpha`
  (pc 14-17), style `'selected'` (pc 18-21); off: `gUnselectedTextColor`, `gUnselectedTextAlpha`, style `'normal'`
  (pc 23-42). PS2 values (interface_util main pc 3-54, §3.1): selected (138,160,244) a 0.6, unselected (240,240,240)
  a 0.6. The 0.6 of the focused row is overwritten by the pulse the next frame [D].
- The bar fades too. The row's bar is the label's flashy background (`ScriptCB_IFFlashyText_SetBackground`,
  `button` main.f7 pc 194-201 etc.), drawn by the flashy-text draw `FUN_00381858` (vtable slot data word 0x62dc48;
  B2 380075-380306) [V]. Its tint: the object colour *(decompiler detail omitted)*, but **(150,150,150, object alpha)** when the mid
  texture is not `headerbuttonmid` AND the text style is not `selected` (B2 380175-380178: `0x96`, hash
  -0x7cdba54a, `+0x68 != 2`) [V]. Style codes from `ScriptCB_IFText_SetTextStyle` (B2 26330-26420): `normal` 0,
  `shadow`/`glow` 1, `selected` 2 [V]. So: unfocused bar = grey 150 x texture at a 0.6; focused bar = object colour
  (default white [H]) x texture at the pulse alpha 0.3..1.0 [D].
- Text colour math (B2 380288-380306) [V]: text RGBA = white (DAT_0076c200 = 255,255,255,255, set B2 712404-712427)
  x the flashy text colour (+0xe4, alpha 255) x the object colour/alpha, each /255 with +0.5 rounding. The object
  alpha byte is `ScriptCB_IFObj_SetAlpha`: 255 when a > 0.99, else int(a x 255) (B2 25384-25410) [V].
  [H] argument 3 is the object's RGBA at +0x40..+0x43 (possibly multiplied by parents); the vtable call site was not
  resolved.
- A selected flashy text WITHOUT a background gets a special colour (255, clamp(1.2 x objAlpha + 50), 100, 255)
  (B2 380276-380286) [V]. No shell button or list row uses that combination (list rows have no style call) [D].
- White vs 240 for unfocused rows [D]: `ShowHideVerticalButtons` re-sets every visible label's flashy text colour to
  255 (110 if dimmed) after `AddVerticalButtons` already applied the select colours (`button` main.f7 pc 266-289); the
  later Enter re-applies the selected colour to the focused row only. Rows not yet visited may show 255 x 0.6 rather
  than 240 x 0.6 until focus passes over them (2-6 levels of difference, hard to see).
- Square command buttons / flat buttons: same `IFButton_fnHilight` path (`ifs_missionselect` Update main.f26 pc 3 =
  the template Update; `SetCurButton` main.f11 pc 56); `bAllSquareButtons` only picks the cap textures (`button` main.f7
  pc 163-239) [V]. `NewFlatIFButton` (`common_ifelem_flatbutton` main.f0) = skin container + IFText label, same
  `IFButton` methods [V]. Icon buttons (`shell_ifelem_iconbutton` main.f5) use the same shared phase but clamp at
  0.5..1.0 and fade text at a, icon at 0.75a; select = text 1.0 / icon 0.75, off = 0.75 / 0.5 (main.f4 pc 0-13) [V];
  no PS2 shell screen in the listings creates one [D].
- Sliders (`common_ifs_opt_sound` Update main.f21): every frame all sliders alpha 0.5, the focused one 1.0; on
  up/down the old slider 0.4, the new 1.0 (main.f18/f19 pc 19, 38). `HSlider_fnSetAlpha` sets bg + thumb (`hslider`
  main.f1) [V]. No pulse. Instant Options (`shell_ifs_instant_options`) has no arrows or sliders: it is a listbox of
  "label: value" text rows (layout main pc 100, PopulateItem main.f2 pc 931-939 sets row colour/alpha) [V].

### Detail: listboxes

- [M menuref §9.1.9] On the PS2 screen the list cursor box does **not** pulse; what follows is the script, whose
  effect is not seen (why is not read).
- `ListManager_fnHilight` (`common_ifelem_listmanager` main.f5 pc 0-26) [V]: max 1.0, min 0.5, step
  `gListbox_CurDir` (start +1.0, main pc 10-13; `gListbox_CurSizeAdd` start 1.0) x fDt; ping-pong; sets the alpha of
  `this.cursor` only. Driven every frame from the template Update (`shellscreen` main.f6 pc 52-67) for
  `gCurHiliteListbox` / `gCurHiliteListbox2` [V].
- The cursor: `NewButtonWindow{ZPos=100, width=row width, height=row height + 10}` with texture `listbox_cursor`
  (`listmanager` main.f0 pc 149-154), a 9-slice with 16-unit corners (`common_ifelem_buttonwindow` main.f0) [V];
  placed at `yTop + (CursorIdx - FirstShownIdx) x (yHeight + ySpacing) + 4` (main.f4 pc 40-57). PS2 skips the PC-only
  mouse `listbox_hilight` box (main.f0 pc 156-174) [V]. Moves are instant (no slide) [V].
- Row text: `ListManager_fnFillContents` (main.f3 pc 48-79) calls the layout's `PopulateFn` with
  (138,160,244, 0.6) for `SelectedIdx`, (240,240,240, 0.6) for the rest; NavUp/Down change `SelectedIdx`, play
  `shell_select_change` and refill via Autoscroll (main.f7) [V]. The Instant Action populate functions apply it with
  `IFObj_fnSetColor` + `IFObj_fnSetAlpha` (`ifs_missionselect` main.f4-f7) [V].
- List windows themselves fade in/out linearly 0 <-> 1 over `gShellAnimTime` = **0.4 s** when the Instant Action
  screen switches columns (`ifs_missionselect` main.f8 pc 0-14; `AnimationMgr_Update` linear lerp `animationmgr`
  main.f2; `shellscreen` main pc 0-1) [V]. That is a show/hide, not the focus highlight.

### Detail: the flashy text effect (native)

- Lua side: `NewIFText` defaults `flashy = 1`, `startdelay = 0` (`interface_util` main.f33 pc 33-41) [V].
  `IFFlashyText_fnSetup(obj, delay, flipV, width, rightJust, extraW)` -> `ScriptCB_IFFlashyText_Setup`
  (B2 26436-26491): delay -> +0xe8, flipV -> bit 35 of +0x100, width -> short +0x100, rightJust -> bit 36, extraW ->
  byte +0x102 [V]. Screen Enter calls `IFFlashyText_StartFlashiness(screen)` which recurses into every child table
  (unless `noTransitionFlash`) (`shellscreen` main.f10 pc 11-13; `interface_util` main.f45) [V].
  `StartFlashiness` native `FUN_00382100` (B2 380311): timer +0xec = 0, phase (bits 32-34 of +0x100) = 0, joins the
  update list [V].
- Timing `FUN_00382158` (B2 380330-380361): each frame timer += dt; while timer >= table[phase]: subtract, phase++;
  at phase 6 leave the list. Table at 0x5ff028 (`re_check.py word b2 0x5ff028 8 --f32`): [0] = this object's
  startdelay (copied in each call), [1] 0.1, [2] 0.05, [3] 0.05, [4] 0.15, [5] 0.15 [V]. Total after the delay 0.5 s.
- What each phase draws, `FUN_00381858` (B2 380075-380306) [V]:
  - phase 0 (start delay): nothing (no bar, no text).
  - phase 1 (0.1 s): the bar only; its middle width scales 0 -> 1 with t/0.1, so the bar **wipes out from the left
    cap to the right** (mirrored if right-justified).
  - phase 2 (0.05 s): full bar + a square of texture `cursor` (side = text height) at the text box's left edge.
  - phase 3 (0.05 s): the `cursor` square **sweeps left -> right** across the text box (x = left + t/0.05 x width).
  - phase 4 (0.15 s): text drawn at full alpha, **height scaled 0 -> 1 about its centre line** (t/0.15).
  - phase 5 (0.15 s): text full size, **alpha 0 -> 255** (t/0.15) - after phase 4's full-alpha squash this reads as a
    blink then fade-in [D].
  - phase >= 6: normal draw.
  So: not a typewriter or scramble; a bar wipe, a sweep square, a vertical pop, then a fade-in [V][D].
- Menu rows: title delay 0, visible row k delay k x 0.1 s (`button` main.f7 pc 115-124 title, pc 183-239 rows,
  `R15 * 0.1` at pc 188; R15 counts visible rows from 1, pc 266). Four-row main menu: title done 0.5 s, last row done
  0.9 s [D]. (AddVerticalButtons first sets `startdelay = i x flashySpeed 0.02`, main.f6 pc 199, but ShowHide re-runs
  Setup with k x 0.1 [V].)
- List rows (`ifs_missionselect` main.f0 pc 27-31 and f1-f3, `ifs_instant_options` main.f1 pc 33): `startdelay =
  math.random() x 0.5`, `nocreatebackground = 1` [V] -> each row starts at a random time in 0..0.5 s, no bar wipe, but
  the `cursor` square sweep and the text pop/fade still run [D: the sweep is drawn outside the background test].
- The first row of an untitled, non-square stack gets flipV = 1 (rounded-top caps from the bottom textures) [V].

### Measured (weak) - existing frames

Single frames (`ref_frames/main_menu*.png`, 640x480) agree but cannot show timing [M]: the focused label's brightest
pixel is blue-tinted and varies between captures - (97,114,144), (121,133,174), (101,114,163) - while unfocused labels
sit at (159-171) neutral grey, close to 240-255 at a 0.6 over the dark bar. A focused alpha between ~0.4 and ~0.7 fits
those numbers; the bar fill samples are swamped by the movie.

## 6. Map list content

Sources. BF2 = `engine/artifacts/lead_tmp/re/bshl/dis/shell_missionlist.txt`, `shell_ifs_missionselect.txt`,
`shell_ifelem_mappreview.txt` (lift pcs in `bshl/lift/` match). BF1 = Lua **4.0** (`\x1bLua\x40`; `lua50dis.py`
refuses it, assert at its line 118). I used my own lister `engine/artifacts/lead_tmp/re/bf2menus/maps/l40.py`, built on
`audit-annc/lua40.py load()`. Listings are in `engine/artifacts/lead_tmp/re/bf2menus/maps/bf1_{missionlist,ifs_missionselect,ifelem_mappreview,ifs_sp_era}.txt`
and cited as `bf1 <file> proto pc`. English text comes from `evmsg/locl.py` (FNV-1a with `c|0x20`) on each game's
`CORE.LVL` 'english' table (BF2 4717 entries, BF1 2053). It reads BF1 too [V] (`engine/artifacts/lead_tmp/re/bf2menus/maps/keys.py`,
`bf1keys.out`, `bf2keys.out`).

### Answer
| | BF2 (PS2) | BF1 (PS2) |
|---|---|---|
| Maps in IA | 24 (13 ground in the table + Mos Eisley + Jabba's + 6 space + ...); full list below [V] `missionlist main pc 174` (sp) = pc 349 (mp), same content | 16 [V] `bf1 missionlist proto[] pc 154` (sp) = pc 309 (mp) |
| Row text | `mapname.name.<code>` English, built at run time (the key is not a table field) [V] `f4 pc 72-76` | `showstr` key per row `planets.<planet>.mapname<n>`, e.g. "BESPIN: PLATFORMS" [V] `bf1 ifs_missionselect proto[2] pc 7` |
| Description | `mapname.description.<code>` in the bottom info box [V] `f6` + `ifs_missionselect main.f9 pc 11-16` | **none**: BF1 IA has no description box; no description key in the table [V] |
| Order | `table.sort` ascending by localized name, raw `<` on `ScriptCB_ununicode` string [V] `missionlist f1 pc 3-14`, `f0 pc 17-20` (`R2 < R3` -> true) | table order, never sorted (no `sort` in either file [V]); the table is already alphabetical by code |
| Last row | "All Maps" (`mapname.name._all_maps_`, desc "Add all maps to the playlist.") appended [V] `f1 pc 103-109`, `f4 pc 23-29` | none |
| Modes | per map+era flags `mode_<m>_<c/g>`; list = gMapModes order of modes the map has in any era, then "All Modes" if more than 1 [V] `f2 pc 123-167` (`1 < r9` pc 161) | Conquest only (missions `<code><side>`); no mode list [V] |
| Eras | `era_c` / `era_g`; list = eras that have the picked mode (or all the map's eras after All Modes), then "All Eras" if more than 1 [V] `f3 pc 87-143` | buttons `new`="CLONE WARS", `classic`="GALACTIC CIVIL WAR" [V] `bf1 ifs_sp_era pc 55`; era -> attacker side letter [V] `bf1 ifs_missionselect proto[11] pc 12-45` |
| Mission name | `string.format('cor1%s_%s', era, mode subst)` -> `cor1c_con` [V] `f1 pc 73` path / `missionlist_GetMapMode/Era` f8/f9 | `mapluafile .. AttackerChar` -> `BES1r` [V] `proto[13] pc 0-26` |
| PS2 differences | none for the list: the per-mode PC expansion is dead code (`if not false`) [V] `f1 pc 24-27`; no platform filter, so space maps are listed on the PS2 [D]; PS2 sets only the font `gamefont_tiny` [V] `ifs_missionselect f34 pc 8-10` | n/a |
| Preview | **none on the PS2 IA screen**. `ifs_missionselect` has no movie (`movieBackground=nil`, main pc 159). `movieX/W` is set (f34 pc 251-265) but no `movieName` exists in any BF2 luac (byte census: `movieName` 0 files, `NewIFMapPreview` only in its own file). `ifelem_mappreview` is never instantiated [V] | movie `<CODE>fly` (e.g. `BES1fly`), played 0.5 s after the cursor lands on a map, in a 380x310 window at (screenW-380+10, screenH-260) [V] `proto[7] pc 35-47`, `proto[19] pc 0-37`, `proto[33] pc 334-356`; the 16 `<code>fly` hashes are present in BF1 `SHELL/ENG/SHELL.LVL` mcfg at 0x462ed1..0x463831 (2-3 hits each; control `bes1` = 0 hits) [V]. The script opens `movies\fly.mvs` (PAL: `flypal.mvs`) [V] `proto[17] pc 12-23`, but the BF1 disc's MOVIES holds only SHELL.MVS + TUTENG.MVS (`analysis/a1-inventory.md:136`), so the engine must map the name [H] |

Text casing [V]: BF1's English table is ALL CAPS ("BESPIN: PLATFORMS", "CLONE WARS"), BF2's is mixed case ("Coruscant").
The PS2 BF2 frame shows CORUSCANT, so that font renders capitals [M, `ref_frames/ia_*`].

### BF2 tables
Modes `gMapModes` [V] `missionlist main pc 572`. The "subst" column is the mission-name suffix.
| key | subst | name key -> English | desc key -> English (first line) | icon |
|---|---|---|---|---|
| mode_con | con | modename.name.con "Conquest" | modename.description.con "Conquest\r\nDefend your command posts..." | mode_icon_con |
| mode_ctf | ctf | modename.name.ctf "2-flag CTF" | ...ctf "Capture The Flag (2 Flag)..." | mode_icon_2ctf |
| mode_1flag | 1flag | modename.name.1flag "1-flag CTF" | ...1flag "Capture The Flag (1 Flag)..." | mode_icon_ctf |
| mode_tdm | tdm | modename.name.tdm "Team DM" | ...tdm | mode_icon_tdm (**not in any BF2 LVL census** [V]; no map uses tdm) |
| mode_hunt | hunt | modename.name.hunt "Hunt" | ...hunt "Hunt\r\nEliminate as many of the indigenous species..." | mode_icon_hunt |
| mode_assault | ass | modename.name.assault "Assault" | ...assault "Assault\r\nDestroy enemy critical systems (18 points)..." | mode_icon_ass |
| mode_eli | eli | **modename.name.assault "Assault"** (so it shows as Assault; [M menuref `ia_allmaps*`: All Maps lists "Assault" twice]) | modename.description.elimination "Assault\r\nHeroes and villains clash sabers..." | mode_icon_ass (`mode_icon_eli` exists in SHELL.LVL but is unused) |
| wildcard | - | modename.name._all_modes_ "All Modes" | ..._all_modes_ "Add all available game modes for this map." | none |

Eras `gMapEras` [V] `main pc 527`: `era_c` = `common.era.cw` "Clone Wars", teams Republic / CIS, icon1 `cis_icon`,
icon2 `rep_icon`; `era_g` = `common.era.gcw` "Galactic Civil War", teams Rebels / Empire, icon1 `imp_icon`, icon2
`all_icon`. The wildcard is `common.era._all_eras_` "All Eras". Era rows draw icon1 (`ifs_missionselect main.f6 pc 23`).
Playlist rows draw the mode icon plus the era icon1 (`main.f7 pc 32/56`). The textures are in `bshl/tex/shell/mode_icon_*.png`
and `tex/common/{all,cis,imp,rep}_icon.png` [V].

Maps in PS2 list order (= ascending English name [D, sort rule applied to the texts below]; the first 9 match the frame
CORUSCANT..KAMINO [M]). Description = 2nd line of `mapname.description.<code>`; line 1 is "<Planet>: <site>".
| # | code | name | site (desc line 1) | CW modes | GCW modes |
|---|---|---|---|---|---|
| 1 | cor1 | Coruscant | Coruscant: Jedi Temple | con ctf | con ctf |
| 2 | dag1 | Dagobah | Dagobah: Swamp | con ctf | con ctf |
| 3 | dea1 | Death Star | Death Star: Interior | con 1flag | con 1flag |
| 4 | end1 | Endor | Endor: Bunker | - | con hunt 1flag |
| 5 | fel1 | Felucia | Felucia: Marshland | con 1flag | con 1flag |
| 6 | geo1 | Geonosis | Geonosis: Dust Plains | con ctf hunt | - |
| 7 | hot1 | Hoth | Hoth: Echo Base | - | con 1flag hunt |
| 8 | tat3 | Jabba's Palace | Tatooine: Jabba's Palace | con 1flag | con 1flag |
| 9 | kam1 | Kamino | Kamino: Clone Facility | con 1flag | con 1flag |
| 10 | kas2 | Kashyyyk | Kashyyyk: Beachhead | con hunt ctf | con ctf |
| 11 | tat2 | Mos Eisley | Tatooine: Mos Eisley | con ctf | con ctf hunt eli |
| 12 | mus1 | Mustafar | Mustafar: Refinery | con ctf | con ctf |
| 13 | myg1 | Mygeeto | Mygeeto: War-torn City | con ctf | con ctf |
| 14 | nab2 | Naboo | Naboo: Theed | con ctf hunt | con ctf |
| 15 | pol1 | Polis Massa | Polis Massa: Medical Facility | con ctf | con ctf |
| 16 | spa7 | Space Felucia | Space over Felucia (large) | ass 1flag | - |
| 17 | spa8 | Space Hoth | Space over Hoth (small) | - | ass 1flag |
| 18 | spa3 | Space Kashyyyk | Space over Kashyyyk (small) | ass 1flag | - |
| 19 | spa6 | Space Mygeeto | Space over Mygeeto (small) | ass 1flag | - |
| 20 | spa9 | Space Tatooine | Space over Tatooine (large) | - | ass 1flag |
| 21 | spa1 | Space Yavin | Space over Yavin (large) | - | ass 1flag |
| 22 | tan1 | Tantive IV | Tantive IV: Interior | con 1flag | con 1flag |
| 23 | uta1 | Utapau | Utapau: Sinkhole | con 1flag | con 1flag |
| 24 | yav1 | Yavin 4 | Yavin 4: Temple | con 1flag | con 1flag |
| 25 | `_all_maps_` | All Maps | Add all maps to the playlist. | | |

The mode column order shown is the table's field order. The screen orders modes by gMapModes (con, ctf, 1flag, tdm,
hunt, ass, eli) [V f2]. All 85 map x era x mode missions exist as `extracted/bf2/lua/mission/*.luac`, 0 missing [V]
(script check against the pc 174 table).

### BF1 table
Name key = `planets.<p>.mapname<n>`; English from BF1 CORE.LVL [V]. Side letters: r = Republic, c = CIS (CW era,
button `new`); a = Alliance, i = Imperial (GCW era, `classic`). When a map has both sides of an era the first one wins:
CW picks `c` else `r`, GCW picks `a` else `i` (`proto[11] pc 16-40`). The mission is the code plus that letter, matched
by the disc's `lua/mission` list and `attract_mode_maps` (`bf1 missionlist pc 355`) [V]. Preview = `<CODE>fly` for every row.
| # | code | key | English | CW mission | GCW mission |
|---|---|---|---|---|---|
| 1 | BES1 | planets.bespin.mapname1 | BESPIN: PLATFORMS | bes1r | bes1a |
| 2 | BES2 | planets.bespin.mapname2 | BESPIN: CLOUD CITY | bes2r | bes2a |
| 3 | END1 | planets.endor.mapname1 | ENDOR: BUNKER | - | end1a |
| 4 | GEO1 | planets.geonosis.mapname1 | GEONOSIS: SPIRE | geo1r | - |
| 5 | HOT1 | planets.hoth.mapname1 | HOTH: ECHO BASE | - | hot1i |
| 6 | KAM1 | planets.kamino.mapname1 | KAMINO: TIPOCA CITY | kam1c | - |
| 7 | KAS1 | planets.kashyyyk.mapname1 | KASHYYYK: ISLANDS | kas1c | kas1i |
| 8 | KAS2 | planets.kashyyyk.mapname2 | KASHYYYK: DOCKS | kas2c | kas2i |
| 9 | NAB1 | planets.naboo.mapname1 | NABOO: PLAINS | nab1c | nab1i |
| 10 | NAB2 | planets.naboo.mapname2 | NABOO: THEED | nab2c | nab2a |
| 11 | RHN1 | planets.rhenvar.mapname1 | RHEN VAR: HARBOR | rhn1r | rhn1i |
| 12 | RHN2 | planets.rhenvar.mapname2 | RHEN VAR: CITADEL | rhn2c | rhn2a |
| 13 | TAT1 | planets.tatooine.mapname1 | TATOOINE: DUNE SEA | tat1r | tat1i |
| 14 | TAT2 | planets.tatooine.mapname2 | TATOOINE: MOS EISLEY | tat2r | tat2i |
| 15 | YAV1 | planets.yavin.mapname1 | YAVIN 4: TEMPLE | yav1c | yav1i |
| 16 | YAV2 | planets.yavin.mapname2 | YAVIN 4: ARENA | yav2r | yav2i |

Other BF1 IA text [V]: "AVAILABLE MAPS:", "SELECTED MAPS:", "PICK ERA:", "CONTINUE" (launch), "PLAY IN ORDER" /
"RANDOM ORDER", "SELECTED MAPLIST IS FULL.". Background `his_brief_BG` [V] `bf1 ifs_missionselect pc 132`. Each
planet also has a bare name key `planets.<p>.name` ("BESPIN"), but IA does not use it.

### Merged list for the port (40 maps + All Maps)
Rule [D]: sort the merged labels case-insensitively. The port title-cases BF1's all-capital English names
to match BF2's mixed-case labels, then appends the same lowercase game tags. "Hoth (bf2)" comes before
"Hoth: Echo Base (bf1)" because the space sorts before the colon. The suffix only breaks
exact-name ties, and there are none. Case-folded order:

| # | row text | code | game | eras | modes | desc source | preview |
|---|---|---|---|---|---|---|---|
| 1 | Bespin: Cloud City (bf1) | bes2 | bf1 | CW GCW | con | none | BES2fly |
| 2 | Bespin: Platforms (bf1) | bes1 | bf1 | CW GCW | con | none | BES1fly |
| 3 | Coruscant (bf2) | cor1 | bf2 | CW GCW | con ctf | mapname.description.cor1 | none |
| 4 | Dagobah (bf2) | dag1 | bf2 | CW GCW | con ctf | ...dag1 | none |
| 5 | Death Star (bf2) | dea1 | bf2 | CW GCW | con 1flag | ...dea1 | none |
| 6 | Endor (bf2) **C** | end1 | bf2 | GCW | con hunt 1flag | ...end1 | none |
| 7 | Endor: Bunker (bf1) **C** | end1 | bf1 | GCW | con | none | END1fly |
| 8 | Felucia (bf2) | fel1 | bf2 | CW GCW | con 1flag | ...fel1 | none |
| 9 | Geonosis (bf2) **C** | geo1 | bf2 | CW | con ctf hunt | ...geo1 | none |
| 10 | Geonosis: Spire (bf1) **C** | geo1 | bf1 | CW | con | none | GEO1fly |
| 11 | Hoth (bf2) **C** | hot1 | bf2 | GCW | con 1flag hunt | ...hot1 | none |
| 12 | Hoth: Echo Base (bf1) **C** | hot1 | bf1 | GCW | con | none | HOT1fly |
| 13 | Jabba's Palace (bf2) | tat3 | bf2 | CW GCW | con 1flag | ...tat3 | none |
| 14 | Kamino (bf2) **C** | kam1 | bf2 | CW GCW | con 1flag | ...kam1 | none |
| 15 | Kamino: Tipoca City (bf1) **C** | kam1 | bf1 | CW | con | none | KAM1fly |
| 16 | Kashyyyk (bf2) **C** | kas2 | bf2 | CW GCW | con ctf hunt(CW) | ...kas2 | none |
| 17 | Kashyyyk: Docks (bf1) **C** | kas2 | bf1 | CW GCW | con | none | KAS2fly |
| 18 | Kashyyyk: Islands (bf1) | kas1 | bf1 | CW GCW | con | none | KAS1fly |
| 19 | Mos Eisley (bf2) **C** | tat2 | bf2 | CW GCW | con ctf hunt(G) eli(G) | ...tat2 | none |
| 20 | Mustafar (bf2) | mus1 | bf2 | CW GCW | con ctf | ...mus1 | none |
| 21 | Mygeeto (bf2) | myg1 | bf2 | CW GCW | con ctf | ...myg1 | none |
| 22 | Naboo (bf2) **C** | nab2 | bf2 | CW GCW | con ctf hunt(CW) | ...nab2 | none |
| 23 | Naboo: Plains (bf1) | nab1 | bf1 | CW GCW | con | none | NAB1fly |
| 24 | Naboo: Theed (bf1) **C** | nab2 | bf1 | CW GCW | con | none | NAB2fly |
| 25 | Polis Massa (bf2) | pol1 | bf2 | CW GCW | con ctf | ...pol1 | none |
| 26 | Rhen Var: Citadel (bf1) | rhn2 | bf1 | CW GCW | con | none | RHN2fly |
| 27 | Rhen Var: Harbor (bf1) | rhn1 | bf1 | CW GCW | con | none | RHN1fly |
| 28 | Space Felucia (bf2) | spa7 | bf2 | CW | ass 1flag | ...spa7 | none |
| 29 | Space Hoth (bf2) | spa8 | bf2 | GCW | ass 1flag | ...spa8 | none |
| 30 | Space Kashyyyk (bf2) | spa3 | bf2 | CW | ass 1flag | ...spa3 | none |
| 31 | Space Mygeeto (bf2) | spa6 | bf2 | CW | ass 1flag | ...spa6 | none |
| 32 | Space Tatooine (bf2) | spa9 | bf2 | GCW | ass 1flag | ...spa9 | none |
| 33 | Space Yavin (bf2) | spa1 | bf2 | GCW | ass 1flag | ...spa1 | none |
| 34 | Tantive IV (bf2) | tan1 | bf2 | CW GCW | con 1flag | ...tan1 | none |
| 35 | Tatooine: Dune Sea (bf1) | tat1 | bf1 | CW GCW | con | none | TAT1fly |
| 36 | Tatooine: Mos Eisley (bf1) **C** | tat2 | bf1 | CW GCW | con | none | TAT2fly |
| 37 | Utapau (bf2) | uta1 | bf2 | CW GCW | con 1flag | ...uta1 | none |
| 38 | Yavin 4 (bf2) **C** | yav1 | bf2 | CW GCW | con 1flag | ...yav1 | none |
| 39 | Yavin 4: Arena (bf1) | yav2 | bf1 | CW GCW | con | none | YAV2fly |
| 40 | Yavin 4: Temple (bf1) **C** | yav1 | bf1 | CW GCW | con | none | YAV1fly |
| 41 | All Maps | `_all_maps_` | - | | | mapname.description._all_maps_ | |

**C** = collision. These codes exist in both games: end1, geo1, hot1, kam1, kas2, nab2, yav1, plus tat2 (BF2 Mos
Eisley vs BF1 Mos Eisley, same place and same code). So the code alone cannot be a key; the port already keys by
`<game>_<code>` (`engine/crates/game/src/assets.rs:131-155`). Same-planet pairs with different codes: Kashyyyk
kas1/kas2, Naboo nab1/nab2, Tatooine tat1 (BF1) / tat3 (BF2), Yavin yav2. Names never collide exactly: BF1 always adds
": <site>" and BF2 shows the planet only, except "Mos Eisley" / "Jabba's Palace" / "Space ...". Bespin and Rhen Var
are BF1-only; the BF2 ground maps without a BF1 counterpart are cor1 dag1 dea1 fel1 mus1 myg1 pol1 tan1 uta1 tat3.

### What the port already bakes [V]
- English text for both games: yes. `bake locl` (`engine/crates/bake/src/hud.rs:211-232`) writes
  `engine/assets_baked/common_bf{1,2}/locl/english.ron` (4717 / 2053 entries, same counts as `locl.py`).
  - Hash check: `mapname.name.cor1` 0x1183729c, `mapname.description.cor1` 0x6cd91fdb, `mapname.name.tat2`
    0x38169ee6 and `spa1` 0x6fb6b78e are present in bf2. `planets.bespin.mapname1` 0x957f9107, `rhenvar.mapname2`
    0x0272e19f and `common.era.cw` 0x58b9455c are present in bf1. Control 0xdeadbeef is absent from both.
  - Text check: "Mos Eisley" 2 / "All Maps" 1 / "2-flag CTF" 3 hits in bf2, "BESPIN: PLATFORMS" 1 in bf1, control
    string 0 hits.
- Mode icons, `mapselect_icon_*`, `iface_instant_action`: **not baked**. `find engine/assets_baked -iname '*mode_icon*'
  -o -iname '*mapselect*' -o -iname '*iface_instant*'` gives 0 hits; control `*hud_icon_flash*` gives 2. The BF2
  shell bake keeps only `logo` (`bake/src/shell.rs:33 SHELL_TEXTURES = ["logo"]`; `common_bf2/shell/textures/` = 1
  file).
- Era icons: `{all,cis,imp,rep}_icon.ktx2` exist only in `common_bf1/textures`; 0 in `common_bf2/textures` (166 files).
- Previews: BF2 has none to bake. BF1 `<code>fly` segments: not baked. No `*fly*` movie under assets_baked;
  `common_bf2/shell/movies` = 10 objm (the shell_main / shell_sub_left loops only).
- Baked maps (top-level dirs, 31):
  - BF1 (15): bf1_{bes1,bes2,end1,geo1,hot1,kam1,kas1,kas2,nab1,nab2,rhn1,rhn2,tat2,yav1,yav2} plus the legacy
    unprefixed `tat1` (manifest source_game "bf1"), so all 16 BF1 maps are present.
  - BF2 (16): bf2_{cor1,dag1,dea1,end1,fel1,geo1,hot1,kam1,kas2,mus1,myg1,nab2,pol1,tan1,tat3,uta1,yav1} plus the
    unprefixed `tat2` (source_game "bf2").
  - Missing: the 6 BF2 space maps (`game/tests/suite/locate_ps2.rs:9` already says space is not baked).
  - Missions (`engine/missions/`, 61 RONs): conquest only, `<key>_con[_cw].ron`. No ctf/1flag/hunt/ass/eli missions.
- The port's current shell list shows the uppercased directory code plus "(bfN)", sorted by directory key, with no
  display names (`engine/crates/ui/src/shell.rs:201-211`, `assets.rs:154`). The legacy `tat1` / `tat2` dirs would
  strip no prefix and sort apart from the `bf1_` / `bf2_` keys [D].

## 7. The port's own calls for these screens

- **Map list = both games** (the one deliberate difference). Row text "<name> (bf1)" / "<name> (bf2)". For BF2
  maps the name is `mapname.name.<code>` ("Hoth (bf2)"); for BF1 maps the BF1 key `planets.<p>.mapname<n>`
  ("Hoth: Echo Base (bf1)" after title-casing). Sort: BF2's ascending sort, case-folded
  (§6 merged table). Description box: BF2 `mapname.description.
  <code>`; BF1 has none on its disc - show line 1 = the BF1 name and no body, or write a short line of our own
  [ours, flag it]. Preview: BF2's IA has none, so none for BF1 either (the BF1 `<CODE>fly` clips stay unused).
- **Dimmed = the PS2's own style** (re-bf2-shell §3.3: label grey 110, focus skips). It exists for button stacks
  only; the PS2's list boxes have no dimmed rows. Extending it to list rows (grey 110 text, Up/Down and the cursor
  skip it) is a port addition [ours]. Use it for: maps not baked (the 6 BF2 space maps today), modes the port cannot
  run (everything but Conquest today: 2-flag / 1-flag CTF, Hunt, Assault) and their "All Modes" row when only one
  mode is live; Session Options buttons whose group the port does not honour (Hero, CTF, Hunt, Assault today) and
  rows inside a group the port ignores.
- **Era**: the PS2 era list stays (Clone Wars / Galactic Civil War / All Eras); the port's cross-era pairing
  (re-bf2-shell §9.6) is an extra row after "All Eras" [ours].
- **AI Difficulty Level = the port's four tiers** (a named deviation, the maintainer 2026-10-28). The PS2 row's levels (1 Easy,
  2 Normal, 3 Elite; only Normal <-> Elite reachable, §2.3) are replaced by **Easy, Normal, Hard, Elite**, one
  step per Left / Right and clamped at both ends like the other rows (L1/R1 go to the ends; no wrap). Easy is the
  PS2's Normal (base skill 14), Normal the PS2's Elite (17), Hard and Elite are ours and harder than anything the
  PS2 has (`engine/crates/core/src/skill.rs` has the table). Normal is the default, so the row reads "< Normal >"
  like the PS2's frame. Words: BF2's own `ifs.difficulty.easy` "Easy", `ifs.difficulty.medium` "Normal" and
  `ifs.difficulty.hard` "Elite" (BF2's word for its top level, here the port's top tier); "Hard" is ours, English
  (BF2 has no "Hard" text). The four names were Normal, Hard, Elite, Legendary for part of 2026-10-28 (same rows,
  one step up); the maintainer renamed them the same day. `ui::shell::SessionOptions::difficulty` holds 1..=4 in that order;
  the value is the profile's tier and is saved when Session Options is left.
- **Launch**: the PS2 hands the whole playlist to the engine (`ScriptCB_SetMissionNames`) and plays it in order; the
  port must queue the playlist the same way (map, mode, era per row) - a single-map launch is a deviation.

## 8. What an implementer has to bake

Checked 2026-10-27 against `engine/assets_baked` of the main checkout (`find -iname '*<name>*'`; control hits:
`border_3_pieces` 2, `bf2_buttons_topleft` 1, `logo` 1). The BF2 bake picks COMMON / CORE sheets by name in
`engine/crates/bake/src/hud.rs` (`SCREEN_SHEETS`, plus the HUD layouts' own names) and SHELL.LVL textures in
`engine/crates/bake/src/shell.rs` (`SHELL_TEXTURES = ["logo"]`). `analysis/re-bf2-shell.md` §8 is out of date: the
button pieces, `btna` / `btnb`, the logo, the two menu movies and the shell music are now baked.

| what | source on the disc | baked today? | needed for |
|---|---|---|---|
| `iface_instant_action` (512x512) | SHELL.LVL | **no** | IA backdrop |
| `mode_icon_con`, `_2ctf`, `_ctf`, `_hunt`, `_ass` (`_eli` unused, `_tdm` absent from the disc) | SHELL.LVL | **no** | Mode rows, playlist rows |
| `imp_icon`, `all_icon`, `rep_icon`, `cis_icon` | COMMON.LVL | BF1 copies only (`common_bf1/textures`) - **no BF2 copy** | Era rows, playlist rows |
| `listbox_cursor` | COMMON.LVL | BF1 copy only | every list's focus box |
| `listbox_topbot_more` | COMMON.LVL | **no** | list scroll arrows |
| `white_rect` | COMMON.LVL | **no** | Session Options info box |
| `cursor` | COMMON.LVL | BF1 copy only | the flashy-text sweep square (§5) |
| `border_3_pieces` | COMMON.LVL | yes (`common_bf2/textures`) | every window |
| `bf2_buttons_{topleft,title_center,topright,midleft,items_center,midright,botleft,botright}` | COMMON.LVL | yes | stacks, window title bars |
| `btna`, `btnb` | COMMON.LVL | yes | help bar |
| `btnmisc`, `btnmisc2` (Square, Circle) | COMMON.LVL | BF1 copies only | help lines on Audio (Square "Reset"), profile, keyboard |
| `optionl`, `optionr` | COMMON.LVL | yes (HUD bake) | the `{OptionL}` / `{OptionR}` arrows in option rows |
| `opaque_black` | COMMON.LVL | yes | `bDimBackdrop` screens |
| `slider_bg`, `slider_fg`, `slider_sound` | COMMON.LVL | `slider_bg/fg` BF1 only; `slider_sound` no | Audio Options |
| `logo` | SHELL.LVL | yes (`common_bf2/shell/textures`) | title, main menu |
| fonts `gamefont_tiny/small/medium/large` | CORE.LVL | yes (`common_bf2/fonts`) | all text |
| English text: `ifs.*`, `mapname.*`, `modename.*`, `common.*`, BF1 `planets.*` | CORE.LVL `Locl` (both games) | yes (`common_bf{1,2}/locl/english.ron`, all keys) | all text |
| movies `shell_main`, `shell_sub_left` | MOVIES/SHELL.MVS | yes (`common_bf2/shell/movies`) | menu backgrounds |
| clicks `shell_select_change`, `shell_menu_accept/enter/exit/cancel/error`, `shell_keyboard_key/backspace` | CORE.LVL bank `global` | samples yes (`core_bf2/sounds`), **played by nothing**; the front's sound library loads only `common_bf2/shell` and so never sees the CORE bank or the bus table (§4.3) | every menu |
| `shell_soundtrack` (4 takes, bus `shellmusic` 0.6) | SOUND/SHELL.LVL stream `shell_music` | yes (raw ADPCM); the port plays take 1 looped without the 0.6 bus gain | menu music |
| `shell_auditionSFX` / `shell_auditionVO` | SOUND/SHELL.LVL stream `shell_vo` | yes (OGG) | Audio sliders |
| BF2 space maps (spa1, 3, 6, 7, 8, 9) and non-conquest missions | map LVLs | **no** (dim those rows) | IA list |

So for the Instant Action screen the bake lacks: `iface_instant_action`, five `mode_icon_*`, the four era icons
(BF2 copies), `listbox_cursor`, `listbox_topbot_more`, `white_rect`, `cursor`. Add the SHELL.LVL ones to
`SHELL_TEXTURES` and the COMMON.LVL ones to `SCREEN_SHEETS` (or a sibling list). For sounds: load `core_bf2` in the
front end's sound library and play the names of §4.

## 9. What still needs a PCSX2 look

None of this was captured for this note (PCSX2 was busy). The 2026-10-28 walk answered all but the column fade, the phase carry-over and BF1 (§10; its README's "§9 answers" table goes item by item). The BSHL save states in `engine/artifacts/lead_tmp/re/bshl/ref_frames/` (title, main menu, Singleplayer, Instant Action, Options) are the starting points; start one with `pcsx2_session.py start iso/SWBF2.iso --state <state> --boot-wait 10`. Pad taps need a 300 ms hold and 1.5-1.8 s between presses.

### 9.1 Instant Action and Session Options (lead)
Start from `bshl/ref_frames/bf2_shell_ia.p2s` (IA, focus ADD, empty playlist); taps need a 300 ms hold and 1.5-1.8 s
between presses.
1. **AI cap**: Down x 3 to OPTIONS, Cross (Session Options), Cross (Global), Right x 12 on "AI Units Per Team".
   Read where it stops (16 or 25, §2.3 [H]); then Left until it stops (expect 0).
2. **Per-mode AI**: Triangle, Down x 2 to Conquest, Cross: read AI Units (16 on the frame); back, CTF and Assault:
   read theirs (expect 16: -1 in the image, raised to 16 by `FUN_0019bfc8`, §2.3). Then set Global AI to 10 and re-read Conquest (expect 10).
3. **Hero page**: Session Options -> Hero: read every row and its default (Heroes On/Off; if On, the unlock rule,
   value, assign, timer). Press Right on "Unlock Hero" (expect Points <-> Timer only).
4. **Session Options info-box font**: on any group page compare the info text's glyph height with `gamefont_small`
   and `gamefont_tiny` (the script passes a nil font, §2.2 [H]).
5. **Options survive**: set Conquest Reinforcements to 150 %, Triangle twice to the IA screen, back into Conquest:
   expect 150 % (ResetDefaults on IA Enter re-reads the engine's stored values, §1.3 [D]).
6. **Era rows**: ADD, Cross on KASHYYYK, Cross on Conquest: count the visible Era rows and measure a row's height
   (expect 38 units, up to 4 rows) and the era icon at the row's right edge (14.25 units, grey-tinted).
7. **Remove**: add HOTH twice (ADD, HOTH, Conquest, GCW; again), then Down x 2 to REMOVE, Cross, Down, Cross:
   expect one HOTH removed and focus back on LAUNCH; again: with only "Remove All" left focus lands on ADD; Cross on
   "Remove All" clears the list.
8. **Command stack geometry**: on `ia_playlist_launch.png`-like frames measure the four square buttons' bar edges
   (HardWidthMax 80, row 21 every 25 units, `yTop` -130 at container 0.6) - the stack's x / y origin is not derived
   in §1.2.
9. **Two pulses at once**: move into the Planet list and capture 60 frames: expect ADD's row pulsing at 0.7 s and
   the list cursor at 1.0 s (§1.3 [D], §5).
10. **Column change fade**: Cross on a planet and capture every frame for 0.6 s: expect the Planet window to fade
    out and Mode in over 0.4 s.
11. **Scroll**: in the Planet list hold Down past KAMINO: the list scrolls by how many rows (the `scrollby` default
    = floor(9 / 2) = 4 on a page, 1 per Down), the up arrow appears.
12. **Full list**: (optional) ADD -> All Maps -> All Modes -> All Eras: count the rows added (85 expected for the
    BF2 table, under the 128 cap; §6).

### 9.2 The rest of the menu tree (§3)
State `bf2_shell_main.p2s` (focus Singleplayer) unless said; taps need a 300 ms hold, 1.5-1.8 s apart.

1. **Game Options**: from `bf2_shell_options.p2s` press Cross (Game Options). Measure: window left edge / width
   (predicted 391.7 units, left-aligned), row height (predicted 48), rows visible (predicted 5), the {OptionL}/{OptionR}
   glyphs, the values on a fresh profile (defaults). Then Down x 9 to see every row and the slider.
2. **Audio Options**: from the Options state: Down, Cross. Measure: label column edge (x 0.4 safe), slider x / width
   (161.3) / height 24, thumb width (0.1 x width), the % text and its value at defaults, slider alpha focused vs not;
   press Square to see the reset popup.
3. **Controls**: from the Options state: Down x 2, Cross -> Select Control Type (stack position, left-justified?);
   Cross -> Infantry: the pad page (picture, callout labels, cursor, preset list after Cross on "Choose Preset").
4. **Online Options**: Down x 3, Cross (does the PS2 open it without a network config? any popup?).
5. **Credits**: Down x 5, Cross: the tilt, speed (40 units/s), colours, first lines.
6. **View Trailer**: Down x 4, Cross: confirm `trail.mvs` plays full-screen and Triangle stops it.
7. **Career Stats**: Down x 6, Cross: the `statsscreens_bg` layout, boxes, fonts.
8. **Singleplayer sub-screens**: from `bf2_shell_singleplayer.p2s`: Down x 2, Cross (Rise of the Empire): the
   training popup (Play / Skip, 400 x 240) then the New Game / Load stack (centre at 0.5); Back; Down x 3, Cross
   (Galactic Conquest): the 5-row stack (Campaign List hidden?), width; Down x 1, Cross (Space Overview): the movie.
9. **Multiplayer** (no network in PCSX2): main state, Down, Cross: the "No Network Adaptor" popup (Popup_Ok_Large
   400 x 240 sizing) - or, with the network adaptor on, "Pick Connection Type" (LAN dimmed without a cable).
10. **Profile screens**: cold boot to a profile with an existing save: the list rows (font, spacing), Square "Delete"
    help and its popup; the keyboard's symbols page (Circle on `profile_name`).
11. **Pause menu**: in Hoth (after the IA launch), Start: rows shown, stack width, the dim backdrop, the Quit text
    (host or plain).
12. **Main menu Back** with a changed profile: the 3-button save popup (Save and Exit / Exit Without Saving / Cancel).

### 9.3 Sounds (§4) - record the audio, not frames
1. Main menu, BF2: press Down x5 (through the wrap) - expect one `shell_select_change` per press, 42 ms, ~0.1 s
   apart at most (playinterval); measure the level vs accept.
2. Instant Action map list: hold Up at the top row - expect silence (no wrap); press Down once - one click.
3. Instant Action, command state: Launch with an empty playlist - listen for error + accept together, and their
   order.
4. Instant Action, command state: Triangle back to Singleplayer - one or two `shell_menu_exit`? Measure the gap.
5. Instant Action options: Left on a value already at its minimum - expect a click anyway; R1 = one click.
6. Options > General: Left/Right on "Heroes" - expect the accept sound, not the select click.
7. Audio options: move the master slider - expect a delayed audition line, no click; check the music slider.
8. Any forward transition (Main -> Singleplayer -> Instant Action) - confirm no whoosh plays in BF2.
9. Shell music: from boot, log which of the 4 `shell_soundtrack` takes plays first, and what follows at its end
   (repeat? next? random?); time the 1 s `shellmusic` fade on a push.
10. Error popup OK (e.g. memory-card error): confirm silence (`shell_menu_ok` undefined).
11. Pause menu in battle: START (accept), move (select), Triangle (exit), Quit -> No (cancel).

### 9.4 The focus pulse and the flashy entrance (§5)
1. Button pulse period and range: `bf2_shell_main.p2s` (focus Singleplayer), no presses; capture 60 consecutive frames
   (1 s at 60 Hz, or every vsync). Read the Singleplayer label's brightest-stroke pixel and a bar-interior pixel left
   of the text (x~175, y~255 of 640x480) per frame; expect a triangle with 0.7 s period, alpha 0.3..1.0 on both.
   Use `main_menu_median4`-style medians of a static movie region or pause the movie if possible.
2. Bar tint focused vs unfocused: same frames; compare the focused bar at its alpha peak with the Multiplayer bar
   (expect white x texture vs 150 grey x texture at 0.6).
3. Phase carry-over: press Down once (300 ms hold); check the new row's alpha continues the old row's ramp (no reset)
   and the old row snaps to a 0.6 grey.
4. Listbox cursor: `bf2_shell_ia.p2s`, move focus into the planet list; capture 90 frames; read the `listbox_cursor`
   box edge pixel; expect alpha 0.5..1.0, 1.0 s period; row text static.
5. Flashy entrance: from the main menu press Cross on Singleplayer (or Triangle back to main) and capture every frame
   for 1.2 s after the press; expect title then rows at 0.1 s steps: bar wipe 0.1 s, square sweep 0.1 s, text squash
   0.15 s, fade-in 0.15 s. Same on the Instant Action screen to see the random 0..0.5 s row starts.
6. dt units: confirm item 1's period is 0.7 s (seconds) and not frame-locked.

### 9.5 Map list (§6)
1. BF2 IA, list scroll [M]: ADD -> Planet list, press Down 24 times. Read every row (confirms rows 10-24 incl. the
   space maps on the PS2 and the casing), then the "All Maps" row and its info-box text.
2. BF2 Mos Eisley mode list [M]: Planet list -> MOS EISLEY -> X. Expect Conquest / 2-flag CTF / Hunt / Assault / All
   Modes (eli shown as "Assault"). Read the mode icons and the info box on the eli row; check whether the playlist
   later shows `tat2g_eli`. [M menuref: the All Maps mode list shows two "Assault" rows (Mos Eisley itself not
  opened); the port lists Assault once.]
3. BF2 era list after "All Modes" on Kashyyyk [M]: expect Clone Wars / Galactic Civil War / All Eras. Read the era
   icon pair drawn on each row (icon1 = cis_icon on CW, imp_icon on GCW).
4. BF1 IA, cursor on BESPIN: PLATFORMS, wait more than 0.5 s [M]. Confirm a `BES1fly` clip plays in the lower-right
   380x310 window, and note which MVS it streams from (resolves the `movies\fly.mvs` vs disc SHELL.MVS [H]).
5. BF1 IA era buttons on END1 (GCW only) and GEO1 (CW only) [M]: confirm the missing era's button is hidden
   (`proto[12]` ShowHideItems), and read the button text casing.

## 10. PCSX2 check 2026-10-28

A PCSX2 2.8.0 walk of the BF2 menus with lossless video and the game's own sound (`engine/artifacts/lead_tmp/menuref/
README.md`: per-screen tables, a §9 answer table, the sound table; clips, frames, sheets and pixel measures beside it)
contradicted the script reading here. Corrected in place above, each tagged [M] with its clip or measure:

1. §0, §5: the button pulse period is **0.734 s** (22 updates at 30 Hz, linear ramps), not 0.7 s.
2. §0, §1.3, §5: the `listbox_cursor` box does **not** pulse; only the focused button row does.
3. §0, §5: the flashy entrance runs faster than the ELF table: wipe ~0.07 s, square ~0.1 s, pop ~0.07 s, fade
   ~0.07 s; rows 0.1 s apart.
4. §0, §2.3: AI Units Per Team **0..16** (not 25); Global copies into Conquest, CTF and Assault; values persist.
5. §0, §2.3: Hero defaults Heroes **Off** (other rows hidden), Unlock Points, value 10, Best Player, Timer **90 s**;
   Assign does not wrap; Timer Never -> Left -> 120.
6. §2.2: the Session Options info text is in the rows' `gamefont_small`, white, on the grey half-transparent box.
7. §1.2, §1.3: lists scroll one row per press past the 9th visible row; L1 / R1 page **8** rows; REMOVE enters
   the playlist on its **last** row.
8. §1.4, §4.1: LAUNCH with an empty playlist is unreachable (dimmed and skipped); Triangle from the IA command
   state is **one** exit click; held directions repeat from ~0.62 s every 0.167 s.
9. §4.1 popups: No / Triangle in a Yes-No popup = cancel; OK popups closed by Triangle = accept; the keyboard's
   Circle = `shell_keyboard_key`; main-menu Triangle with a dirty profile = no sound; Select Difficulty's Cross =
   exit.
10. §4, §4.4: the shell music is a random take each time with repeats, ~10 s apart, from Press START, carried across
    screens.
11. §3: a one-letter profile name is accepted; Singleplayer is always re-entered on Training; Game Options' Tool Tips
    default Auto (Auto -> Off -> On); Audio Output Mode Stereo -> Dolby Pro Logic II -> Mono, defaults Master 100,
    Music 60, SFX 70, Speech 90, Battle Speech 100 %; the single-player Quit text is the host one.
12. §6: All Maps lists "Assault" twice (eli shares the name) - a PS2 quirk the port does not copy.

The port follows these in `ui::shell_feel` (pulse, flashy, music, hold repeat), `ui::shell` (ranges, defaults,
lists, the Instant Action clicks), `ui::shell_ia` (the stack pulse, the info text), `ui::shell_ps2` (Training focus)
and `game::pause::pause_sound`. Not applied, the port having no such screen: Audio Options' Output Mode and its PS2
defaults, Game Options' Tool Tips, the profile and keyboard screens.
