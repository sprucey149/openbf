# OPUS RE (2026-09-30) - [O] UNAUDITED
# The PS2's on-screen event messages: kill, death, command posts, points — both games (card EVMSG)

Corpora: D: = `analysis/SWBFRNT2.ELF-decomp.c` (BF2), B: = `analysis/SWBFRONT.ELF-decomp.c` (BF1). Text = the English
`Locl` table of each game's CORE.LVL (`[u32 fnv(key)][u16 size][UTF-16]`, a2-bake-formats §Localization). Layout = BF2's
`1playerhud` dump `engine/artifacts/lead_tmp/re/bhud/dump_named.txt` (card BHUD). PS2 = two live PCSX2 battles (software
renderer) sampled from EE RAM by the game clock, BF1 KAS2 (Clone Wars, Republic) and BF2 HOT1 (Empire); frames and states
in `engine/artifacts/pcsx2/ref/evmsg/`. Tools in `engine/artifacts/lead_tmp/re/evmsg/`: `locl.py` / `keys.py` (hash ↔
text, which ELF strings are keys), `constscan.py` (a 32-bit constant anywhere in an ELF), `evram.py` (the widgets' RAM
layout), `evsample.py` (the live sampler), `teamcol.py` (team colour tables), `fn.py`, `checks.txt` (the pasted
re_check runs), `claims.md` + `VERDICTS.md` (the verifier).
Tags: [V] read (lines cited), [D] derived, [H] hypothesis, [M] measured on the PS2 (RAM or frame).
Hops: (1) the BF2 layout's `player1.message` listener → its three senders → their 14 call sites; (2) the game-event
dispatcher of each game and the kill / command-post triggers; (3) the two display widgets (BF2 MultiLineText, BF1
MessageDisplay) and the statistic panels; (4) both Locl tables; (5) PS2 RAM + frames for position, colour, timing;
(6) `re_check.py` fn / absent / callers / word / writers (`checks.txt`) + `constscan.py`; (7) one foreground Opus
verifier, 29 claims: 25 SUPPORTED, 4 PARTLY, 0 NOT SUPPORTED — folded in (§8).

## Headline
- **Neither game has a kill feed.** A soldier's death makes an event line only when the victim is a human player or a
  hero (and, for a hero AI victim, only offline on one screen with a human or hero killer). Killing an ordinary AI
  soldier shows **no text** in either game [V][O]; what the player sees is his **statistics panel**: BF1's three icons
  (kills, deaths, posts) flashing yellow for 3 s, BF2's points + kills numbers with a "+1" delta for 5 s + 1 s fade [V][M].
- **BF1** draws event lines bottom-right, right-aligned, tilted 30° like the rest of its HUD, `gamefont_tiny`, two lines
  at most, each exactly 4.0 s, no fade [V][M]. Lines: `%s KILLED %s`, `%s KILLED TEAMMATE %s`, `%s KILLED HIMSELF`,
  `%s DIED`, `%s CAPTURED THE %s`, `%s LOST THE %s`, `%s JOINED`, `%s LEFT`. Colour = the event team as the viewer's team
  sees it: green (100,200,100) own, red (200,100,100) enemy, grey (192,192,192) nobody [V][M].
- **BF2** draws them upper-left under the weapon block in the layout's `player1systemMessages` MultiLineText: four lines,
  `gamefont_tiny` with a shadow, each 0.5 s fade-in, 3.0 s hold, 0.5 s fade-out, the rest scrolling up [V][M]. Colours:
  blue (1,86,213) own team, red (200,100,100) enemy, grey neutral. **BF2 has no command-post line at all**: its events
  4–7 only play the post's voice line; the strings `%s captured the %s` / `%s lost the %s` are still in its Locl table
  but no code uses them [V][O]. The same element also carries the hero lines, squad orders (cyan), award lines
  (cyan / yellow), Lua's `ShowMessageText` (yellow) and the network join / leave / boot lines.
- AI soldiers have names: BF2 HOT1 "Ben Pulastra killed Player 1", BF1 KAS2 "UNIT 688 KILLED PLAYER 1" [M].
- Corrects `re-cp-capture.md` §4/§7: "HUD `game.infomsg.captured` / `lost`" is BF1 only.

## 1. The event (both games) [V]
One record per event, made by B `FUN_00307940` (B:291943) / D `FUN_003e0218` (D:438824): type +8, subject player +0xc,
other player +0x10, command post +0x14, team object +0x20; shown locally by B `FUN_00307a00` (B:291974–292135) /
D `FUN_003e0320` (D:438867–439084) (offline directly; a client replays the server's, D `FUN_003dfeb8` D:438700).

| type | made by | BF1 line (key string) | BF2 line (Locl hash) |
|---|---|---|---|
| 0 suicide | the death handler, killer == victim | `game.infomsg.suicide` "%s KILLED HIMSELF" (B:292016) | 0x616333ea "%s killed himself"; a hero: 0x07aad4c1 "%s has fled" with the hero class's name (D:438906–438915) |
| 1 team kill | same team | `game.infomsg.teamkill` "%s KILLED TEAMMATE %s" (B:292021) | 0xc57ae533 "%s killed %s - [TK]" (D:384597) |
| 2 kill | other team | `game.infomsg.killed` "%s KILLED %s" (killer, victim) (B:292025–292028) | 0xacc1eef7 "%s killed %s"; a hero victim: 0x083ab1e9 "%s defeated %s" (killer, hero class name) (D:384596–384612) |
| 3 died (no killer) | | `game.infomsg.died` "%s DIED" (B:292031) | 0x0a9f4178 "%s died"; killed by an object of class `DAT_0068c8f0`: "%s killed %s" with 0x664ffe37 "Auto Turret" as the killer; a hero: "%s has fled" (D:438921–438945) |
| 4 post captured | the post tick (`re-cp-capture.md` §4) | `game.infomsg.captured` "%s CAPTURED THE %s" (team name, post name) + the post's voice line (B:292035–292057) | **no text** — `FUN_003570b8` (voice line) only (D:438946–438955) |
| 5 post lost (neutralised) | | `game.infomsg.lost` "%s LOST THE %s" + voice line (B:292058–292080) | **no text** — voice line only |
| 6, 7 dispute / saved | | voice line only (B:292081–292090) | voice line only (D:438956–438965) |
| 8, 9 joined / left | network | `game.infomsg.joined` / `.left` | 0xdd433dcf "%s joined" / 0xf2898f4f "%s left" |
| 10, 11 | BF2 boot vote | — | 0xd6a2b4a1 "%s was booted for %s." / 0xd5a2b30e "%s was not booted." |

- BF1's team name is the wide string at team+8 (KAS2: "REPUBLIC", "CIS", "WOOKIEES" [M]), else the ELF string
  "Undefined" (0x542298); the post name is the string at CP+0x1c, else Locl 0x7e64bc51 "COMMAND POST" (B:292039–292052).
  PS2 KAS2: "REPUBLIC LOST THE PIER ONE" (green), 33.2 s later "CIS CAPTURED THE PIER ONE" (red) [M].
- **BF2 has no post line** [V][O]: cases 4–7 of `FUN_003e0320` are two calls of `FUN_003570b8` (D:351332–351477), which
  plays sounds and sends no HUD event; hashes 0xdd279cfc / 0xae1ec790 occur nowhere in BF2's ELF (`constscan.py`: 0 code,
  0 data hits; the scanner finds every hash of the table above) nor in the decomp; no `infomsg` string on the BF2 disc
  (verifier: ELF and five LVLs). What BF2 shows for a capture: the minimap and world markers, the voice line (card ANNC),
  the points pop-up (§5) and — at the post the player stands on — the tooltip "This CP is now controlled by your team."
  (hash 0x64bcc6d2, the tooltip table at 0x5ff4ac ff., the bottom bar `player1tooltipsTEXT`) [M frame; its trigger [H]].
- Unused in both games: Locl 0xf0d356d2 "%s killed %s (%s)" (no key string in BF1's ELF, no constant in BF2's) [D].

### 1.1 Who gets a kill line — the gate [V][O]
B `FUN_00297440` (B:222068–222160), D `FUN_0034f650` (D:345853–346268; the flag starts at D:346114):
```
show = victim is human                                   # controller index >= 0 (B +0x9c, D +0x130)
if offline and viewports == 1:                           # *(DAT_00517420+0x20) / *(DAT_005fc4ec+0x20) [V verifier: the
    show |= killer is human or killer is a hero          #   split-screen count: a loop bound over local players]
show &= victim is human or victim is a hero              # hero: B char+0xd0 = class+0x7c at spawn (B:221523);
if show: event(type 0 | 1 | 2, victim, killer, ..., team) #       D char+0x165 = class+0x330 bit 33 (D:345041)
```
So: the player's own death always; a human killing a human; a human or hero killing a hero AI (one screen only). An
ordinary AI's death: nothing. BF2's `FUN_0034da30` ("is a hero") is also true for the team's registered hero player with
no unit (D:344418–344441). With no killer the type is 3 and the gate is "victim is human" alone.
PS2 [M]: BF1 pause-menu respawn → "UNIT 688 KILLED PLAYER 1" (red; the last damager is credited) or "PLAYER 1 DIED"
(grey); an enemy's kill → "UNIT 273 KILLED PLAYER 1"; BF2 → "Player 1 died" (grey), "Ben Pulastra killed Player 1" (blue).
170 s of AI battle beside the idle player produced no other line in either game.

### 1.2 Whose screen, which colour [V][M]
Every local player gets every line; the colour is per viewer:
- BF1 (B:292118–292133): bytes at `viewer team + 0x68 + 4 × event team index`; the event team is the killer's
  (`team[record+0x28]`, B:222100) or, for posts, the capturing / losing team.
- BF2 kill lines (D:384617–384621): `viewer team + 100 + 4 × the VICTIM's team index`; the other lines (D:439046–439062)
  the event's team (+0x20) for types 0 and 3, the subject player's team for 8–11.
- The tables in RAM [M] (`teamcol.py`): BF1 own (100,200,100), enemy (200,100,100), index 0 (192,192,192); BF2 own
  (1,86,213), enemy (200,100,100), index 0 (192,192,192) — the HUD's reinforcement colours (re-hud §3).
- BF2's kill lines (types 1, 2 only) are skipped for a player whose HUD object's bytes +0x18 / +0x19 are not both 1
  (D:384592; ctor sets both, D:384474); +0x18 is cleared while his pause menu is open (`FUN_00389b48`), +0x19 has no
  live writer [V verifier].

## 2. BF2's display — `player1systemMessages` [V data][V][M]
Layout (dump 1755–1777): `MultiLineText`, EventText `player1.message`, EventColor `player1.message.color`, NumLines 4,
DisplayTime 3, AlwaysScroll 1, `#09771c74(1)`, Rect (0.5, 0.25) Left / Top, Position (0.079938, 0.191374) Viewport,
ZOrder 0; Format: TextBox (0.5, 0.25), word break, TextStyle Shadow, alignment Left / Center, `gamefont_tiny`, colour
(192,192,192). Events registered per local player by `FUN_003b0aa8` (D:408081–408086).
- **Senders** [V][O]: only `FUN_003b7398` (D:412125, a team mask + a Locl hash), `FUN_003b74a0` / `FUN_003b7528`
  (D:412159 / 412178, one player + a ready string, reached through `FUN_003898f0` D:384531); each sends the colour event
  first. Verifier's own ELF scan: exactly three sites load the event slot.
- **MultiLineText** (ctor `FUN_003aa3e0` D:403692, properties `FUN_003ab628` D:404414): NumLines → +0x194, DisplayTime →
  +0x1a8 (default 2.0), ScrollSpeed (0xdfc96107) → +0x3a4 (default 1.0), AlwaysScroll → +0x3a8 bit 0, 0x09771c74 → bit 1,
  0xcf649993 → bit 2 (the last two names not recovered).
  - Add (`FUN_003ab1e0` D:404273): free = NumLines − (pending +0x19c + shown +0x1a0); **free 0 → the message is
    dropped**; else the text is word-wrapped and at most `free` lines are queued, each with the current colour.
  - Admit (`FUN_003aa908` D:403920): when idle and the newest shown line leaves room (`FUN_003aa8b0`), the next pending
    line is placed at y = shown × line height (bit 1 set — lines stack downward from the top; bit 1 clear: at the box's
    bottom line) and faded in (state 4 → shown += 1 when its fader reaches hold).
  - Expire (`FUN_003aa620` D:403755): the top line goes when its fader has held longer than DisplayTime — always with
    AlwaysScroll, else only when lines are pending and (bit 1 clear or the box is full). It fades out (state 2), then
    the remaining lines scroll up (state 3, `FUN_003aa9f8` D:403959) at line height × ScrollSpeed × clamp(y / line
    height, 0.5, 1) per second; bit 2 hides the element when it empties.
- **Timing on the PS2** [M] (HOT1, RAM by the game clock, `ref/evmsg/bf2_hot1_samples.jsonl`): queued 216.537 → hold
  217.071 (**0.534 s**) → fade starts 220.040 (**2.969 s**) → gone 220.574 (**0.534 s**) → idle one frame later. The
  lines' faders hold FadeIn 0.5 / Hold ∞ / Sustain 0 / FadeOut 0.5 in RAM (the Format block names none; the tooltip's
  Format states the same 0.5 / 0.5) — the 0.534 is 0.5 s on a 30 Hz update.
- **Position on the PS2** [M] (640×480 state frames): "Ben Pulastra killed Player 1" x 49–359, y 90–98, colour
  (1,86,213) + shadow; "Player 1 died" x 50–187, y 89–98. re-hud §2.4's mapping puts the element's origin at
  (48.1, 84.7); the box fits 7 lines (+0x198 [M]) in 0.25 of the viewport → a line ≈ 18 px, the text centred in it
  (centre 93.7 predicted, 94 measured).
- **Other users of the same element** [V]: the hero spawn 0x0f669681 "%s is now playing as %s" (player, hero class) in
  the hero team's relation colour (`FUN_003825c8` D:380606–380641); squad orders to the ordering player in cyan
  (32,223,223) `DAT_0076c240`: 0x731fd493 "Follow me", 0x0e0280e5 "Move out", 0xa0590ed7 "Hold on" (`FUN_00240bf8`
  D:161347 / 161454 / 161558 / 161696), 0xf00699ad "Get in" (`FUN_002e1670` D:269092), 0x7e3c4096 "Get out"
  (`FUN_002e1ef0` D:269247); awards: Locl key `game.awards.medal.<name>` in cyan and `game.awards.bonus.<name>` in
  yellow (`FUN_00360320` D:357432, callers D:357223 / 357423 / 357540); Lua `ShowMessageText(key [, team])` in yellow
  (223,223,32) `DAT_0076c250` (D:33896–33961) — in Conquest only KAS2's "level.kas2.objectives.gateopen" (the mission
  bytecode), otherwise CTF and campaign; the CTF flag resets (`FUN_0035e058`); a network text line (`FUN_003f2b08`).
  Named colours: `FUN_0057ca38` D:712396 ff., RAM [M].

## 3. BF1's display — `MessageDisplay` [V][M]
One per local player (`DAT_00564628[i]`, RTTI `14MessageDisplay`). Ctor `FUN_002d0370` (B:259118–259168): **two** text
lines; the group at (−30, −40, 40) under the anchor (1.0, 1.0) of the viewport (`FUN_002d05e0` B:259215), font
`gamefont_tiny` (`FUN_002d0588`, string 0x540408); in split screen the y offset comes from the table at 0x540450
(−40 / −40) (`FUN_002d0768` B:259285).
- Add (`FUN_002d0b10` B:259388): with two lines in use the oldest is dropped at once (scroll = 1.0); the new line takes
  the HUD time and the colour.
- Update (`FUN_002d0838` B:259314): the oldest line is dropped when its time + **4.0** < now (only while the scroll is
  0, verifier); line i sits at y = (i + scroll) × −20 — the newer line ABOVE the older; scroll falls at 2 per second
  (0.5 s slide). No fade: a line is shown or hidden.
- PS2 [M] (KAS2): the line vanished 4.004 s after it appeared (clock 1202.989 → 1206.993). RAM: the group's matrix is a
  30° rotation about X with translation (−30, −40, 40) — BF1's HUD is a tilted 3-D plane; right-aligned (text extents
  −100…−2 × 5…19 for "PLAYER 1 DIED"). Frame (640×480): "PLAYER 1 DIED" x 457–595, y 439–454; "REPUBLIC LOST THE PIER
  ONE" x 300–596, y 439–455 → 1.42–1.43 px per HUD unit across, the anchor at x ≈ 643 (the screen's right edge). This is
  the BF1 HUD unit → pixel scale `re-hud-icons.md` left open (one depth, z = 40).
- A second caller, `FUN_00312288` (B:299070), prints "<name>: <text>" (network chat) in (200,100,100).

## 4. BF1's statistics panel — `StatIconDisplay` [V][M]
One per local player (`DAT_00564638[i]`). Ctor B:263184–263283: three rows 32 units apart at the anchor (0, 0.5) of the
viewport (`FUN_002d7320` B:263359), each an icon 32 × 32 (`stats_kills`, `stats_deaths`, `stats_cpp`) and a
`gamefont_small` number (`FUN_002d7220` B:263326), row scale 0.75. Update `FUN_002d7530` (B:263473–263604): values =
kills − 2 × team kills (`FUN_002a64e8`), deaths (`FUN_002a6618`), posts captured (`FUN_002a6660`); when one changes the
row takes yellow (223,223,32) if it rose, red (200,100,100) if it fell, its flash timer = 1.0 and the panel timer = 3.0;
a row keeps the flash colour while its timer ≥ 0.5 and blends to the base (white at alpha 64) over the last 0.5 s; the
panel is drawn while its timer > 0, alpha 255 down to timer 1.0, then timer × 255.
PS2 [M]: deaths 0 → 1 one frame after the death line; panel timer 2.97 at the first sample, alpha 255 for 2.0 s, then
254 → 7 over 1.0 s, off at +3.0 s. Frames: `ref/evmsg/bf1_kas2_killed_line_xshot.png` (skull + "1" yellow), the
`bf1_kas2_gameplay.mp4` fight (kills 1, 3 yellow then dim).

## 5. BF2's statistics pop-up — `player1stats` [V data][V]
Layout (dump 2489–2622): Group at (0.098092, 1.15511) Viewport, Alpha 0.6, FadeHoldTime 5, FadeOutTime 1, enabled by
`player1.statistic.changed`; inside, icon `points` + number `player1.statistic.points` + text `…pointsDelta`, icon
`stats_kills` + `…kills` + `…killsDelta` (`gamefont_small`, black drop-shadow twins, the delta (190,190,190) with
ColorPulseRate 0.75). No deaths / captures / hero-points element in `1playerhud`.
Publisher `FUN_003b5de8` (D:411375–411457): five statistics (names at 0x5ffa30: points, kills, deaths, heropoints,
captures; kills = kills − 2 × team kills, `FUN_00363d28`); for each that differs from the last value sent: the number
event, the Delta text `%s%d` of (new − last) with "+" when it rose (0x631f50 / 58 / 60), then one `changed` event.
PS2 frame [M] (`ref/evmsg/bf2_hot1_cp_tooltip_xshot.png`): the kills icon at ≈ (72, 227), the points star at ≈ (72, 256)
of 640×480, "−1 −1" after a suicide.

## 6. Names [M][D]
Each team object loads up to 64 first and 64 last names from Locl keys `<side>.firstnames.%02d` / `.lastnames.%02d`,
`<side>` = `common.sides.all|imp|rep|cis|neu` by the team's faction id (D `FUN_0036d880` D:366896; the same strings in
BF1, `FUN_002ad2f0`). Both tables hold: `all` 48 first + 48 last (Rayce, Jayce, Han … / Branigan, Nordan, Bodalla …),
`rep` 24 first (Alpha, Beta, Gamma …) and no last, `cis` one first ("Unit"), `imp` and `neu` none. Seen: "Ben
Pulastra" (Rebel), "UNIT 688", "UNIT 273" (CIS). The three ways a name is composed are measured in §9.2 (the keys
count from 01); the composer's code was not read [H]. The human is "Player %d" (0x62a708 / B 0x53ed90).

## 7. For the build (port decisions — reversible, not PS2 facts)
1. One renderer for both games' maps: BF2's `player1systemMessages` and `player1stats` from the baked `1playerhud`
   (BHUD2's interpreter needs a MultiLineText: §2's queue, admit, expire, scroll; fader 0.5 / 3.0 / 0.5).
2. Feed, per local player, coloured per viewer (BF2's table: blue own, red enemy, grey neutral): §1's lines under §1.1's
   gate, from BF2's Locl by hash (bake the English table: `[hash → text]`); the hero lines; the statistics (§5).
3. **Post lines on every map** — the maintainer's INBOX asks for them and PLAN says BF1's event in BF2's style where BF2 lacks
   it: "%s captured the %s" / "%s lost the %s" (BF2's own unused strings, 0xdd279cfc / 0xae1ec790; team name, post
   name else "Command Post"), colour by the event team. The BF2 PS2 shows none (§1): a per-game switch reverses it.
4. Squad orders (SQMK / SQACK) and awards (AWDM) are lines of the same element: cyan (32,223,223), yellow (223,223,32).
5. AI names: `<first> <last>` where the side has last names, else `<first> <n>`; PROVISIONAL (§6).
6. Not here: tooltips (the bottom bar, card TIPS), "Leaving battlefield" (card BNDY), the voice lines (card ANNC).
Acceptance numbers: §2's and §3's boxes and times; reference states `ref/evmsg/bf2_hot1_kill_line.p2s`,
`bf2_hot1_died_line.p2s`, `bf1_kas2_lost_line.p2s`, `bf1_kas2_died_line.p2s` (each holds its frame and RAM).

## 8. Checks, verifier, what stays [H]
`checks.txt`: `re_check.py fn` on 56 cited lines (each in the function named here); `absent` — `DAT_006a938c` in 5
functions (register, free, the three senders), `dd279cfc` / `ae1ec790` / `game.infomsg` 0 hits in D:; `callers` of the
senders and of both event makers (BF2 10 call sites, BF1 8); `word` for every key string, the two "%s%d" strings, the
−40 table; `writers` of the named colours. `constscan.py bf2`: every BF2 hash of §1 found at its call site, the two
post hashes and 0xf0d356d2 at none.
Verifier (claims + citations only, its own searches; `VERDICTS.md`): 25 SUPPORTED, 4 PARTLY, 0 NOT SUPPORTED of 29.
Folded in: the hero branch of type 3; the colour team per case (types 8–11 use the subject player's team); the admit
step needs room; BF1 drops an expired line only at scroll 0; `FUN_0034da30`'s second way to be a hero; +0x18 is the
pause menu's, +0x19 is never written; the viewport-count reading; ScrollSpeed's name; the delta is new − last. Its
falsification attempts found no fourth sender and no other path for post text.
[H] / not read: the two unnamed MultiLineText bits' names; where the 0.5 s line fades are set (RAM only); the name
composer (§6); the tooltip system (its table at 0x5ff4ac, its triggers); BF1's HUD camera (only the scale at z = 40 is
measured); `FUN_003f2b08`'s line; a kill made by the player was not captured live (the gate is [V], the BF1 panel's
kills row is on video); split-screen layouts (`2playerhud` … carry their own `playerN.message` elements).

## 9. Addendum (2026-10-02, card EVMSG build) — points, names, the line's glyph pitch, the pop-up's place
Tools in `engine/artifacts/lead_tmp/evmsg/`: `points_ram.py` (the points table from a state), `names_ram.py` (every
player record's name), `postnames.py` (post-name keys over every baked map). Verifier (one foreground Opus agent, the
8 code claims of §9.1, citations only): 6 SUPPORTED, 2 PARTLY, 0 NOT SUPPORTED — its narrowings are folded in.

### 9.1 BF2's points [V][M][O]
- The "points" number of §5 is the s16 at stats+0x1fc plus the s16 at +0x1fe (the hero's) (`FUN_00361900`
  D:358433–358442, read by `FUN_003b5de8` D:411405–411408; signed loads, verifier).
- `FUN_003616f0(stats, kind, x)` (D:358334–358404) adds a SIGNED byte of the table `DAT_005fedf0[kind]` (D:358359) to
  +0x1fc, or to +0x1fe when `FUN_0034da30` says hero (D:358381–358400). The table is empty in the ELF and has one
  writer, `FUN_00366080` (D:361555–361564) ← `ScriptCB_SetPlayerStatsPoints` (D:48450): the scripts fill it.
  **In RAM [M]** (`points_ram.py`, both HOT1 states): `0:1 1:2 2:1 3:-1 4:-2 5:3 6:2 7..11:1 12:10 13..19:1 20:5 21:5
  22:2 23:2 24:2 25:1 26:1 27:2 28:10 29:2 30:2 31:-12 32:10 33:2 34:2`.
- The death handler `FUN_0034f650` sends an ENEMY victim to `FUN_00362268` and a FRIENDLY one (the killer himself
  included) to `FUN_00362c60` (D:346219–346228).
  - `FUN_00362268` (D:359253–359268): kind **0** (+1) for an AI victim (controller index +0x130 < 0), kind **1** (+2)
    for a human, kind **0x14** (+5) for a hero — unless a special case fired first, which REPLACES it: kinds 0x17 / 0x18
    (at most one, a 16-post loop with `break`) and 0x1e / 0x1b / 0x1d (a list walk, each can fire per entry), +2 each.
    What those cases are (a kill beside a post, within 20 m …) was not read [H].
  - `FUN_00362c60` (D:359388–359430): counts a kill AND a team kill (so §5's "kills − 2 × team kills" falls by 1),
    then kind **3** (−1) when the victim is the killer, else kind 0x1f (−12) if a list entry's +0x300 is the victim
    (not read [H]), else kind **4** (−2).
  - No killer player and no killing team (D:346145–346192): the VICTIM gets kind **3** (−1). The PS2 frame
    `ref/evmsg/bf2_hot1_cp_tooltip_xshot.png` shows it: kills 0, points −1 with "−1" [M].
- A capture (`FUN_00357440`, after the type-4 event, D:352125–352158): every counted unit in the region with a player
  gets *(decompiler detail omitted)* — kind **0x15** (+5) for that one player, kind **0x16** (+2) for the
  others, and +1 capture each (D:359947–359970). *(decompiler detail omitted)* is the player with the LARGEST time since his +0x19c
  stamp for this post (D:351807–351846; strictly positive — a player stamped this frame cannot win; the scan has more
  conditions than the award loop, verifier) [D]: "longest in the region".
- All of it is gated by `DAT_005fede9`, `FUN_004241a0()` and `FUN_003e4a30()` (verifier) — not read [H].

### 9.2 Names [M]
`names_ram.py` on `ref/evmsg/bf2_hot1_kill_line.p2s`: 38 player records (`*(0x661330)`, 0x1a4 each), the name a wide
string at +0x30. The human "Player 1"; the 18 Imperial AI **two capitals and three digits** ("DX 178", "KL 987", "TF
997" …); the 19 Rebel AI **first + last name** ("Ben Pulastra", "Thrak Farelle", "Scur Jal'daan" …). With BF1 KAS2's
"UNIT 688" (§1.1): a side with both lists → first + last; first names only → name + number; no list → letters +
number. The Republic (24 first names, no last) was not seen [H]. The Locl keys are numbered from **01**
(`common.sides.all.firstnames.01` = "Rayce" … `.48`), not 00.

### 9.3 Post names [D]
`level.<world name>.<Label>` — the world name as the map LVL's own (`hoth`, `kamino1`, `kas2`), the post's `Label`
property — names 227 of the 248 capture-region instances of the baked maps (`postnames.py`); BF1 `level.kas2.cp5` is
"PIER ONE", the name the PS2 printed (§1). How the game builds the key was not read [H].

### 9.4 The line's glyph pitch [M]
In `ref/evmsg/bf2_hot1_kill_line.png` the 24 glyphs of "Ben Pulastra killed Player 1" start at x 49, 62, 74, 94 … 355:
the font's advances **plus one texel per glyph** (predicted 48, 61, 72, 92 … 354; a constant +1.5 px, no drift over
310 px). Plain advances end at 327, a 1.112 text scale drifts 5 px. So a line of this box is set one texel wider per
glyph than a Text element's ("147" in `re-hud.md` §2.4 fits plain advances). [H] it is the Format's TextStyle Shadow
that adds it (a default character spacing of the element would do the same).

### 9.5 A group's Rect offsets its children [D]
`player1stats` is `Rect(1, 1, "Left", "Center")` at Position (0.098, 1.155): with the rect hung on the origin like a
bitmap's (−0.5 of the viewport down) the kills icon centres at (72.3, 224.9) and the points icon at (72.8, 253.8) of
640×480 — the PS2 frame has ≈ (72, 227) and (72, 256) (§5). Without it they would be at y ≈ 463. The only other
1-player group with a hung Rect is the objective pop-up (Center / Top), where either reading covers the screen.

### 9.6 Built (card EVMSG, 2026-10-02) and what is the port's
`ui/src/hud_tree.rs` (`MultiLine`), `ui/src/hud_draw.rs`, `ui/src/hud.rs` (`feed`), `game/src/event_feed.rs`. The
port's own, each reversible: post lines on every map (`POST_LINES`); the order line with its unit count; a BF1 post
name in title case; which name an AI draws (a hash of its unit index, the PS2 uses its RNG); an identical line is
held back while the first is up; the HUD tree is drawn under the spawn screen so the death line is seen. Not built:
the hero lines and the 2-point kills of §9.1, BF1's two-line MessageDisplay and its StatIconDisplay (BF2's HUD on
every map, PLAN).

## 10. Space Assault objective lines (2026-11-05)

Built from the disc's script; the keys, the voice and the once-per-change rule are in `re-space-assault.md` §13.
