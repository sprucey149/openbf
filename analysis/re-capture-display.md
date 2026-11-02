# OPUS RE (2026-10-13) - [O] UNAUDITED
# The PS2's capture display: what the HUD shows while the player stands in a command post (card CPBAR)

Corpora: B2 = `analysis/SWBFRNT2.ELF-decomp.c` (BF2), B1 = `analysis/SWBFRONT.ELF-decomp.c` (BF1); the ELFs under
`extracted/`; BF2's HUD layout chunks as dumped by `engine/artifacts/lead_tmp/re/bhud/huddump.py`
(`dump_named.txt`). Live: PCSX2 sessions of both games (BF1 Yavin 4 Arena, BF2 Mygeeto), the player parked in a
post's capture region, one row per ~30 ms of the post's RAM and of the HUD object, video beside it, save states at
the interesting moments. Kit: `engine/artifacts/lead_tmp/cpbar/` - `ps2/cp_probe.py` (the probe), `ps2/out/` (rows,
videos, frames, states), `sheets/` (the picture strips), `tools/` (`rows.py`, `strip.py`, `iconbox.py`,
`ramdisp.py`; the ELF scans `handles.py`, `sendscan.py`, `constscan.py`, `tblscan.py`), `checks.txt` (the re_check
output), `verify/` (the claim batches the verifiers were given, their results).
Tags: [V] read (lines cited), [D] derived (from what), [H] hypothesis, [M] measured on the running PS2 (rows, RAM,
frames), [O] load-bearing.
Hops: (1) the six `player%d.commandPost.*` event names in BF2's ELF -> who registers them, who sends them (nobody);
(2) BF1's ELF strings `hud_flag_timer` / `14CaptureDisplay` -> BF1's display class, read whole; (3) BF2's twin by the
texture's name hash 0x621cb6df -> read whole, the apply function disassembled; (4) the scene draw's anchor rule;
(5) live: three BF1 scenarios and five BF2 scenarios (capture, leave and return, contested on either side's meter,
the player's own post attacked), rows + frames + RAM; (6) the five `re_check.py` self-checks (section 8);
(7) the verifier workflow (section 9).

## Headline
- **Both games draw a capture icon in the 1-player HUD.** `re-hud.md` section 3 said BF2's 1-player HUD has no
  capture bar; that was wrong (corrected there). The icon is not in BF2's `hud_` layout data at all: it is a small
  object in the game's code, the same class in both games (BF1's RTTI name is `CaptureDisplay`), one per local
  player. It draws the command-post symbol `hud_flag_timer` (a 64 x 64 white texture whose alpha is the ring with
  four spokes; the same bytes on both discs) at the right edge of the screen at mid height, cut in two by a
  horizontal line: the part above the line in one colour, the part below in another. [V][M][O]
- **What it says** (one rule in both games; the colours are the viewer's team table: neutral (192,192,192), his own
  team BF2 (1,86,213) / BF1 (100,200,100), an enemy (200,100,100)): [V][M][O]
  - a NEUTRAL post: below the line the colour of the team the capture meter belongs to, above it neutral grey; the
    line sits `1 - capture / CaptureTime` of the way down, so the fill RISES from the bottom as the capture goes on;
  - an ENEMY post: below the line the owner's colour, above it grey; the line sits `neutralize / NeutralizeTime` of
    the way down, so grey comes DOWN over the owner's colour until the post is neutral (all grey); then the player's
    colour rises;
  - CONTESTED (a neutral post with two sides inside; the capture meter is frozen): the part above the line
    alternates every half second between grey and the colour of the other side (the first team inside that is not
    the meter's); while the meter is still 0 the whole icon blinks;
  - an enemy post with defenders inside: no sign of them - the neutralize meter is not slowed by defenders and the
    icon does not blink;
  - the player's OWN post: the icon is not drawn, even while enemies neutralize it under his feet;
  - outside every capture region, dead, with the pause menu up, after the battle has ended: not drawn.
- **The 10-segment ring of BF2's `4playerhud` is dead data on the PS2.** The ring (`playerNcpstatus`) is authored for
  the 3- and 4-viewport layout only; the ELF registers HUD events for two players and no code sends any of the six
  `commandPost` events the ring listens to [receipt:2472aa51]. In a 1-player battle the six event objects exist
  with no listener. [V][M]
- For the build (section 10): draw the PS2's icon by the PS2's rule; the maintainer's two asks the PS2 does not answer - his
  own post being lost, and which way the line moves - are additions of ours.

## 1. BF2: the six commandPost events are registered and not sent
- [V][O] `FUN_003b0aa8` (B2:407851 ff.) registers the HUD's events. Inside its per-player loop (stride 0x31c, table
  at 0x6a91f8; the loop runs for player indices 0 and 1 only: *(decompiler detail omitted)* B2:408216) it registers
  `player%d.commandPost.charge` (0x631c50, type 4 = float), `.disable` (0x631c70, type 1), `.color` (0x631c90,
  type 7), `.disputeColor` (0x631cb0, 7), `.disputeEnable` (0x631cd8, 1), `.disputeDisable` (0x631d00, 1)
  (B2:408171-408181) and stores the handles at 0x6a94bc..0x6a94d0 (+0x31c for player 2).
- [V][O] No code sends them [receipt:2472aa51]. The legs, each attacked by two verifiers:
  (i) the decomp names the handle slots only in `FUN_003b0aa8` and in the release `FUN_003b1d50`
  (B2:409266-409305, `… every reference to the table in `.text` forms exactly its base
  0x6a91f8 (51 sites) and the only loads at the six slots' displacements off it are those two functions;
  (ii) the six format strings are named only at B2:408171-408181 [receipt:98110821];
  (iii) an event is sent by building a value object with one of `FUN_003b0480` .. `FUN_003b0500` (the handle is
  register a1) and handing it to `FUN_003b0538` -> `FUN_003b0228` (the one `jal` to the dispatcher, 0x3b0544);
  `tools/handles.py b2` traces a1 at every `jal` to those constructors in `.text`: 236 sites, 229 a plain
  `lw a1, disp(base)`, 0 of them at a commandPost slot (table offsets 0x2c4..0x2d8, absolute low halves
  0x94bc..0x94d0); the 7 others, read in the decomp, take lock-on, weapon and health handles from other offsets of
  the same table (B2:409662, 409850, 410579, 410720, 410782, 410818; the tool's own trace of two of the seven is
  wrong - a `jal` that is a branch target - which does not change its count);
  (iv) by NAME HASH: `FUN_003b0338(hash)` has two callers; `FUN_003b7e10` is the elements' listener attach and
  `FUN_003b7e70` (B2:412660-412682) is a find-or-CREATE reached from the layout keys `eventoutput` (0x6903ed32,
  `FUN_003ba798` B2:414413) and 0xb8e23cbb (`FUN_0039e1a0` B2:396343): a layout element can create and send an event
  by name. The disc's 14 `eventoutput` lines (all in `hudtransforms`) name weapon, health, energy, jet and vehicle
  events only, and no constant in the ELF equals a commandPost event's name hash (`tools/constscan.py`);
  (v) the delayed-send queue (`FUN_003b07b0` / `FUN_003b08b8`, 32 entries at 0x6a8eb8) is used by `FUN_0038abd8`
  alone, for the handles at 0x6a93a0 / 0x6a93a4.
  Limit: a handle copied into another structure and sent from there would escape (iii); no site does that.
- [M] Live (BF2 Mygeeto, 1 player; `ps2/out/bf2_myg1_a.log`; re-read by a verifier from three save states): the six
  event objects of player 1 exist (hashes 9c4703ec, 1dcd875c, 162a6a97, ff91e42f, 834c8957, 6d28d184 = FNV of the
  names; types 4, 1, 7, 7, 1, 1) and their listener lists are empty before, during and after a capture; the
  controls `player1.health`, `player1.healthFraction` and `player1.map.spawnLargeDisable` have listeners.

## 2. BF2: the ring in the layout data (not what the PS2 shows)
- [V] The chunk `4playerhud` (INGAME.LVL 0x321760; `fileinfo("4playerhud") { viewports(3, 4) }`; the 1- and
  2-player chunks say `viewports(1)` and `viewports(2)`) holds three groups `player{1,2,3}cpstatus` (dump lines
  10030, 11995, 13960; all `Viewport(2)`). The first: Position(1.01112, 0.643913), rotation(0, 0, 270), Scale 2.01,
  FadeIn 1 / FadeOut 1, EventEnable `player1.commandPost.charge`, EventDisable `.commandPost.disable`, with two
  BarSegmented rings of 10 segments over 360 degrees in Rect(0.09293, 0.106314): `player1cpdisputebar` (value 1 =
  always full, colour (192,192,192), EventColor `.disputeColor`) under `player1cpcapturebar` (EventValue
  `.commandPost.charge`, colour (223,60,60), EventColor `.color`; its segments fade out in 0.25 s); a segment is the
  `white` bitmap, 0.003122 x 0.014606. The other two sit at (0.867097, 0.634732) with Scale 2.02 and at
  (0.92772, 0.593871) with no Scale. No element listens to `.disputeEnable` or `.disputeDisable`.
- [V] The 1- and 2-player chunks hold no capture element: `commandPost`, `cpstatus`, `cpdisputebar` and
  `cpcapturebar` occur only in the 4playerhud chunk (in the raw file `cpstatus` 3 times, at 0x32fa04, 0x33f1a8 and
  0x34e94c), and no line of any chunk names `hud_flag_timer` - its one occurrence in the file, at 0x1a8f8, is the
  texture itself, far outside the `hud_` chunks (0x2ddbd8-0x350d48) [receipt:ba96375a].
- [D] The ring cannot light on the PS2: its events are not sent (section 1), `player3.*` events are not even
  registered, and the listener attach does not create a missing event. What `.charge` would have run from and to is
  not in the PS2's code.

## 3. The capture display (BF2; BF1 beside it)
| | BF2 | BF1 |
|---|---|---|
| objects | `FUN_0037e040` (B2:377639-377659): two of 0x2f0 bytes at `DAT_006613a0[0..1]`, from the level init `FUN_004241b0` (B2:481791) | `FUN_002bd808` (B1:249064-249085): two of 0x200 bytes at `DAT_00564618[0..1]`, from `FUN_0033e590` (B1:327847) |
| constructor | `FUN_0037e0a8` (B2:377661-377696): vtable `DAT_0062d8a0`, a group at +0x30, bitmaps at +0xc0, +0x170, +0x220; the group at (0, -30), the second bitmap at (0, 64) (B2:377683, 377688) | `FUN_002bd8c8` (B1:249111-249142): vtable `DAT_0053f948`, group +0x30, bitmaps +0xc0, +0x150; the same two positions (B1:249132, 249137) |
| textures | `FUN_0037e278` (B2:377716-377727): hash 0x621cb6df = `hud_flag_timer` on the first two, 0x7f0688e4 = `hud_notallowed` on the third | `FUN_002bda48` (B1:249161-249179): the string `hud_flag_timer` (0x53f938) on both |
| placing | `FUN_0037e2d8` (B2:377729-377748): scale (0.75, 0.75) when there is more than one viewport; anchor (1.0, 0.5) with flag 1 (`FUN_00565248`) | `FUN_002bdae0` (B1:249181-249200): the same (`FUN_00479f00`) |
| update | vtable slot +0xc = `FUN_0037e400` (word 0x62d8ac; B2:377777-377909); no `jal` reaches it | slot +0xc = `FUN_002bdc30` (word 0x53f954; B1:249246-249352) |
| a neutral post | `FUN_0037e968` (B2:377975-378002) | `FUN_002be058` (B1:249407-249434) |
| an enemy post | `FUN_0037eab0` (B2:378004-378019) | `FUN_002be1a0` (B1:249436-249451) |
| apply | `FUN_0037e780` (B2:377945-377973) | `FUN_002bdf00` (B1:249388-249405) |
| meter fractions | `FUN_00357038` = cp+0xa4 / CaptureTime, `FUN_00357068` = cp+0xa0 / NeutralizeTime (B2:351290-351330; the times through the class pointer cp+0xb3c) | `FUN_0029ed50` = cp+0x88 / CaptureTime, `FUN_0029ed80` = cp+0x84 / NeutralizeTime (B1:227733-227770; class cp+0x690) |
| over the network | `FUN_003ec7a0` (B2:446950-446999) | `FUN_0030f580` (B1:297276-297306) |

### 3.1 The update [V][O]
(BF2 offsets; BF1's in brackets.) Each update of the object:
1. It finds its player (BF2 `FUN_003d6db8`, the 0x1a4-byte player table `DAT_00661330`; BF1 `FUN_002acac0`). If the
   object is switched off (its byte +0x20, section 3.4 - BF2 only: BF1's update does not read it), the player has no
   character (player +0x148 [BF1 +0xb4] is 0) or the battle has ended (`FUN_004241a0()` = `DAT_00600b3c` [BF1
   `FUN_0033e580()` = `DAT_00519084`]) it detaches from the player, hides and returns; otherwise it attaches.
2. A session gate (B2:377829-377843): c = `DAT_00600548`, or `DAT_00600549` when `DAT_0060054f` is set; when c is
   non-zero the walk of step 3 runs only if `DAT_00600562` is set and (`DAT_006ccae0` is set or `DAT_006d85b4` >=
   0). [BF1: the walk runs only when `DAT_00518be4` (or `DAT_00518be8` when `DAT_005e9a70` is set) is 0.] c is the
   network flag: `DAT_00600549` [BF1 `DAT_00518be8`] is set to 1 by the two functions `ScriptCB_SetAmHost` calls.
   On a machine the gate refuses, the display is driven by a message instead: a kind (hide / neutral post / enemy
   post), the team words and the fraction in 6 bits (x 1/63) into the same two functions (BF2 `FUN_003ec7a0`,
   BF1 `FUN_0030f580`). [M] all gate flags 0 in an Instant Action battle (RAM, both games; ELF defaults 0x600548 = 0,
   0x600549 = 0). Not part of the port.
3. It hides the group (BF2 `FUN_0037e760` B2:377845; BF1 clears the drawn bit, B1:249298), then walks the command
   posts (the 16-slot table the iterators `FUN_0035ba28` / `FUN_0035ba08` [BF1 `FUN_002a2670` / `FUN_002a2650`]
   step through) and takes the FIRST post whose mask of human players inside (cp+0x74 [+0x64]) holds the player's
   bit (B2:377850, B1:249303). That mask is the capture tick's: players who count for the capture
   (`re-cp-capture.md` 4). Owner = the post entity's team (B2:377861, B1:249314). The second mask (cp+0x78, players
   inside who do not count) is not read.
4. Owner 0 (neutral): `chg` = the capture meter's team tag (cp+0x7c [+0x68]). `other` = 0 when the post's
   "capturing team this tick" (cp+0x50 [+0x40]) is >= 0; when it is -1 (two sides inside) `other` = the first team
   index from 1 up that is not `chg` and has units inside (cp+0x54 + 4t [+0x44 + 4t]) (B2:377866-377902,
   B1:249317-249346; 8, one past the table, when there is none - not reachable while two sides are inside). Then
   the neutral-post function with x = capture / CaptureTime: split = 1 - x; top colour = table[0] when `other` is 0
   or `(int)(2 x clock) & 1` is 1, else table[other]; bottom colour = table[chg] (B2:377986-377998,
   B1:249418-249430). The clock is seconds since the battle started (BF2 `FUN_004240f8`, BF1 `FUN_0033e4f0`), so
   the top alternates every 0.5 s.
5. Owner not 0 and not the player's team: the enemy-post function with x = neutralize / NeutralizeTime: split = x,
   top colour = table[0], bottom = table[owner] (B2:377880-377882, 378010; B1:249324-249326, 249442).
6. Owner = the player's team: neither function is called and neither meter is read; the group stays hidden.
7. The tail re-arms a hint timer (BF2 `FUN_003943b0(DAT_00661358, 9 or 8, player)` B2:377890: 9 in a post of his
   own team, 8 in another, only while a post holds him; `FUN_003943b0` B2:391030 copies +0x20 to +0x1c of a 0x1c-byte
   entry when offline. BF1 `FUN_002dbcf0(DAT_0056449c, 0xc, in a post?)` B1:249334, every update, a fade state
   machine). Not part of the icon.

### 3.2 The colour table [V][M]
The table is the player's TEAM object's row of 8 colours, one per team index as that team sees it (BF2 team +0x64,
`FUN_0036d9f8` B2:366999-367012 as `re-hud-map.md` 3 reads it; BF1 team +0x68, `FUN_002ad490` B1:238529-238550: by
the relation byte, enemy -> `_DAT_0069c448`, neutral -> `DAT_0069c428`, friend -> `DAT_0069c450`). [M] RAM, team 1's
table: BF2 [0] (192,192,192), [1] (1,86,213), [2..7] (200,100,100); BF1 [0] (192,192,192), [1] (100,200,100),
[2..7] (200,100,100). The minimap's post icons take the same row (`re-hud-map.md` 3).

### 3.3 The apply [V][O]
With f = the split (BF2 +0x2d4, BF1 +0x1e4): the first bitmap takes the top colour, the texture's rows 0..f and the
rectangle 64 wide x f x 64 tall hung from its top right corner (alignment codes 2, 0); the second bitmap, placed 64
lower, takes the bottom colour, the texture's rows f..1 and 64 x (1 - f) x 64 hung from its bottom right corner
(codes 2, 2); then the group is shown (B2:377956-377970, B1:249397-249402). Alignment code 0 = from 0 to +size,
1 = centred, 2 = from -size to 0 (BF2 `FUN_005a0120` B2:735261 ff., BF1 `FUN_004b5e28` B1:576018 ff.). So the two
bitmaps are one 64 x 64 picture of the texture cut at row f x 64.
- [V] BF2's third bitmap (`hud_notallowed`, 64 x 64, colour `DAT_0076c200` = white [M]) is not drawn on the PS2
  [receipt:b2604f6c]: its own drawn bit (bit 8 of +0x48) is set by the constructor and cleared by the apply on
  both branches (0x37e910-0x37e92c: `and v1,v0,-257` / `sd`, twice) before the only call that shows the group;
  showing a group sets bit 10 on its children, not bit 8, and the group draw (0x5a2168, a hole of the export) skips
  a child whose bit 8 is clear (0x5a2264); the two objects are referenced only by their own functions, the level
  init and the network reader. [M] bit 8 = 0 in the mid-capture state. BF1 has no third bitmap. (BF2's network
  message has kinds 3 and 4 that do what 1 and 2 do - [H] the sign's leftover.)

### 3.4 When it is switched off [V]
`FUN_0037e6d8(player, flag)` [BF1 `FUN_002bde68`] writes the object's enable byte and detaches at once on 0; its
four callers (B2:385845 `FUN_0038b590`, 385924 `FUN_0038b908`: `flag = byte +0x18 ^ 1`; 386094, 386119
`FUN_0038bb98`: 0; BF1 B1:260731, 260790, 260876, 260899) switch it together with nine other code-drawn HUD pieces.
`FUN_0038b590` is the pause menu's open / close: it writes its own byte +0x18 and pushes the screen `ifs_pausemenu`
/ `ifs_pausemenu%d` (strings 0x62e568 / 0x62e578, B2:385816-385821) - so the icon is off while the pause menu is
up; `FUN_0038bb98` switches all ten off when `FUN_004241a0()` is set. `DAT_00600b3c` is cleared by the level init
and set in the game's update `FUN_00425840` alone (four places) - [D] the battle has ended. [H] BF1's callers are
the same menu (not read; BF1's update re-attaches on its next run, so how BF1 stays off there is not known).

## 4. Where it sits on the screen
- [V] The group is a root of the HUD scene with anchor (1.0, 0.5) and flag bit 0 set. The scene draw maps a flagged
  root's anchor into a rectangle of the scene: `scene[2] + ax x scene[4]`, `scene[3] + ay x scene[5]`
  (BF2 `FUN_005a3d40` B2:737950-737956; BF1 `FUN_004b8e78` B1:578350-578356); [M] the rectangle is
  (0.05, 0.05, 0.9, 0.9) (RAM 0x7d5f78). So the anchor is (0.95, 0.5) of the frame. The group's bit 1 of +0x88 is
  clear (its constructor clears both bits, `FUN_00565248` writes bit 0 only), so it is drawn with the plain scene
  matrix (`re-hud.md` 2.4: 1.0221 frame pixels across per unit, one frame row down per unit).
- [D][O] On the PS2's 512 x 448 frame the 64 x 64 picture is **x 420.99..486.40, y 194.0..258.0**: its right edge at
  0.95 x 512, its width 64 x 1.0221, its top 30 rows above mid height. With two or more viewports the group is
  scaled by 0.75 about that corner [V, not seen].
- [M][O] Frames (640 x 480 captures; the texture's alpha covers columns 5..59, rows 4..58): the model puts the
  visible symbol at x 532.6..602.9, y 212.1..271.1; measured to a fraction of a pixel on a one-colour frame
  x 532.58..603.16, y 212.71..272.00 (BF1) and x 532.56..603.00 (BF2); `tools/iconbox.py` gives one integer box,
  x 533..601, y 213..270, on six frames of both games. No fitted parameter.
- The same root rule gives the BF1 squad cross's centre x (anchor (1, 1), offset -120): 0.95 x 512 - 120 x 1.0221 =
  363.75, where `re-squad-orders.md` 2b measured 363.8 and left the rule [H] at 366.4.

## 5. Live: BF1 (Yavin 4 Arena, the player on team 1; one enemy bot as the actor) [M]
Rows: `ps2/out/bf1_yav2_*.csv`; strips: `sheets/bf1_capture_neutral.png`, `bf1_leave_enemy.png`, `bf1_contest.png`.
(The rows have holes of 1 to 4 s where a save state was taken, and a few torn rows; the statements hold for the
recorded rows.)
- Neutral post (CaptureTime 10): drawn from the update after the player's bit enters the post's mask; split =
  1 - capture / 10 one update behind (0.917 at 0.87 s, 0.506 at 4.97 s, 0.002 at 10.0); top (192,192,192), bottom
  (100,200,100); hidden one update after the owner becomes his team.
- Enemy post (NeutralizeTime 36, CaptureTime 16): split = neutralize / 36 (0.165 at 5.97 s, 0.826 at 29.76 s), top
  grey, bottom (200,100,100). He leaves at 16.2 s of meter: the meter is 0.00 on the next update (BF1's reset) and
  starts from 0 when he returns. At 36.0 the post is neutral and the bottom turns green at split 1.0. He leaves at
  7.21 s of the capture meter: it holds for 5 s and goes on from 7.24.
- Contested (neutral, his meter at 3.00, the enemy bot placed inside): capturing team -1, the meter frozen at 3.00
  for 10 s, the bottom stays green and the top alternates (200,100,100) / (192,192,192) every 0.5 s.
- His OWN post with the enemy inside: the neutralize meter rises 0 -> 10.3 in 10.3 s and the icon is not drawn in
  any row. He steps out; the enemy neutralizes it and starts his own capture meter (2.27). He steps back in (both
  inside): split 0.773, bottom red (the enemy's meter), top alternating green / grey. The enemy leaves: the meter
  falls 2.27 -> 0 at 1.0 a second (the red shrinks), flips to his team at 0, and the bottom turns green.

## 6. Live: BF2 (Mygeeto, the player on team 1) [M]
Rows: `ps2/out/bf2_myg1_*.csv`; strips: `sheets/bf2_capture_neutral.png`, `bf2_leave_enemy.png`, `bf2_contest.png`,
`bf2_contest_my_meter.png`.
- The same icon at the same place: neutral post, top grey, bottom (1,86,213), split 1 - capture / 10; an enemy post,
  bottom (200,100,100), split neutralize / 12; not drawn on his own post.
- He leaves an enemy post at 5.43 s of neutralize meter: it HOLDS 5.43 for the 5 s the post is empty and goes on
  from 5.47 (BF2's rule, `re-cp-capture.md` 4).
- An enemy post with a defender inside: capturing team -1, the meter rises at 1.0 a second all the same (3.67 ->
  10.93 in 7.3 s), the icon does not blink (top grey in all 226 rows).
- After his capture the neutralize meter is not 0: it reads 11.87 and falls at 1.0 a second while he stands inside,
  holds 2.00 while the post is empty - `re-cp-capture.md` open question 1, seen. With an enemy inside his new post
  it rises again from 9.03 and the post is neutral 3.0 s later; the icon is not drawn meanwhile.
- His own capture meter contested (3.03, the enemy placed inside for 9.4 s): frozen at 3.03, split 0.697, bottom
  blue, top alternating (200,100,100) / grey every 0.5 s - as BF1.
- A neutral post, both inside, the capture meter 0 and no team tag: split 1.000, the whole icon alternates
  (1,86,213) / grey every 0.5 s (the "other" side is then the first team inside: his own). With the enemy's meter at
  4.00: split 0.600, bottom red, top alternating blue / grey; the enemy gone, his presence drains it to 0 at 1.0 a
  second, then the bottom turns blue.

## 7. What stays [H] or unread
(a) what `DAT_00600562`, `DAT_006ccae0` and `DAT_006d85b4` are in BF2's network gate; (b) how BF1 keeps the icon
off under its pause menu; (c) the 0.75 scale with two viewports was not seen (one pad); (d) BF2's `.charge` range
for the ring (no sender on the PS2); (e) where the hint timers 8 / 9 / 0xc show; (f) BF1's scene matrix constants
were not read - the BF1 box is measured, equal to BF2's; (g) what message kinds 3 and 4 are for.
PROVISIONAL for code: the frame box of section 4 (ELF and RAM constants combined; measured to under a pixel).

## 8. Checks run (`engine/artifacts/lead_tmp/cpbar/checks.txt`)
1. `re_check.py fn`: 50 B2 lines and 38 B1 lines, each in the function this doc names (e.g. `B2:377850 is in
   FUN_0037e400`, `B2:377987 is in FUN_0037e968`, `B2:408216 is in FUN_003b0aa8`, `B2:737955 is in FUN_005a3d40`,
   `B1:249303 is in FUN_002bdc30`, `B1:249419 is in FUN_002be058`, `B1:578355 is in FUN_004b8e78`).
2. `re_check.py absent` (the searches behind the absences above): [receipt:2472aa51] the six handle slots: B2 hits
   in `FUN_003b0aa8` and `FUN_003b1d50` alone, 0 in B1, 0 raw words in either ELF; [receipt:98110821] the six
   string addresses: B2 hits in `FUN_003b0aa8` alone; [receipt:4155490e] `hud_flag_timer` / `CaptureDisplay`: 1
   each in BF1's ELF, 0 in BF2's, 0 in both decomps; [receipt:ba96375a] INGAME.LVL raw: `cpstatus` 3,
   `cpdisputebar` 3, `cpcapturebar` 3, `hud_flag_timer` 1; [receipt:b2604f6c] `DAT_006613a0`: 8 lines in
   `FUN_0037e040`, `FUN_0037e6d8`, `FUN_0037eb70`, `FUN_003ec7a0`, `FUN_004241b0`; [receipt:5c394a99] BF1:
   `commandPost.`, `cpdisputebar`, `barsegmented`, 0x4b4bf31a: 0 in B1 and in BF1's ELF (BF2 stores 0x4b4bf31a as
   the segmented bar's type tag in `FUN_003a0240`, B2:397602).
3. The twin: both games' functions opened side by side (the table of section 3); callers by `re_check.py callers`
   and `scripts/tools/refs.py` (the two updates are reached through their vtable slots alone).
4. `re_check.py word`: 0x62d8a8 = 0x0037e200, 0x62d8ac = 0x0037e400; 0x53f950 = 0x002bd9e0, 0x53f954 = 0x002bdc30;
   0x53f938 = "hud_flag_timer", 0x53f968 = "14CaptureDisplay"; the six event format strings at 0x631c50..0x631d00;
   0x62e568 = "ifs_pausemenu". Immediates (`tools/constscan.py`): 0x621cb6df at 0x37e284 and 0x37e2a4, 0x7f0688e4
   at 0x37e2b4.
5. `re_check.py writers`: `DAT_00600548` (5 writes: 0, or a copy of `DAT_00600549`), `DAT_00600549` (1 in
   `FUN_00400030`, `FUN_00400058`, `FUN_004066c0` and, a verifier found, at 0x40bc24 in a function the export
   lacks; 0 elsewhere), `DAT_0060054f` (`FUN_003d4b70`, `FUN_003d5110`), `DAT_00600562` (`FUN_00170770`),
   `DAT_00600b3c` (0 in `FUN_004241b0` / `FUN_00426c58`, 1 four times in `FUN_00425840`); BF1 `DAT_00518be4`,
   `DAT_00518be8` (1 in `FUN_00322f20` / `FUN_00322f48`), `DAT_005e9a70`, `DAT_00519084`. [M] all 0 in the saved
   states (0x600562 = 1, its ELF default).

## 9. Verifier workflow (`cpbar-re-verify`, 2026-10-13; claims and citations only)
8 agents, 44 claims in 6 batches (`verify/batch*.md`): the BF2 object (8), its update (8), its gates and the third
bitmap (5), the BF1 twin (9, read by two agents), the ring and the absences (8, read by two agents), the live rows,
frames and RAM (6, reproduced with the kit's tools and the agent's own scripts). 61 verdicts: **51 SUPPORTED, 10
PARTLY, 0 NOT SUPPORTED**. The two readers of a batch disagreed on 2 claims (the BF1 update and the no-sender
absence: one SUPPORTED, one PARTLY); in both the PARTLY reader upheld the claim's substance and added facts. All 8
PARTLY claims were re-stated above with what the verifiers found; none was refuted and none downgraded:
- the update's hide and walk sit behind the session gate, and a second driver of the display exists - the network
  message of section 3.1 step 2 - found by three agents; the gate's flag is `ScriptCB_SetAmHost`'s;
- two more send routes for HUD events (the `eventoutput` find-or-create, the delayed queue), both empty for the
  ring, and the tool's wrong trace of 2 of its 7 hand-read sites (section 1);
- "the third bitmap is not drawn" needed the group draw's own test of a child's bit, read in an export hole, and
  the objects' full reference list (section 3.3);
- 0x4b4bf31a is stored as a type tag, not compared; the display's field list lacked the class pointer cp+0xb3c;
  the probe's third control event was `map.spawnLargeDisable`, not `map.mode`;
- BF1's update does not read the enable byte BF2's reads; `other` can fall out of its loop as 8 in both games;
- the rows' holes and torn rows; and BF2's own-meter contest was missing from the rows - captured after the
  workflow (`ps2/out/bf2_myg1_e_*`, section 6).
`verify/results.txt` and `results_full.txt` hold every verdict and note.

## 10. For the build (port decisions - reversible, not PS2 facts)
- **Draw the PS2's icon.** `hud_flag_timer` is baked for both games (`common_bf2/textures/`, `common_bf1/`,
  byte-identical). One 64 x 64 picture cut at row split x 64, two quads; on the 512 x 448 frame x 420.99..486.40,
  y 194..258; Hor+ as the squad cross does it (`ui::hud_squad::screen`): the 4:3 picture keeps its place at the
  window's right edge. Code-drawn beside the layout tree, the way `ui/src/hud_squad.rs` is.
- **The rule** as one pure function of (the viewer's team and unit, the post's owner / capture / capture team /
  neutralize, its two times, the units inside per team, the battle clock): section 3.1 steps 3 to 6. The post is
  the first whose capture region holds the viewer's unit while he counts (alive, `capture_posts`). The units inside
  per team come from the sim through one read-only helper that `Sim::update_cps` uses too (no new state: the sim
  hash does not move). Colours: BF2's table on every map (`ui::hud::HudMessage::{OWN, ENEMY, NEUTRAL}`, the
  minimap's) - the port's HUD is BF2's everywhere (PLAN); BF1's own HUD drew its friend green.
- **Ours, for the maintainer's asks the PS2 does not answer** (each a named deviation with a FEEL_NOTES line and a switch):
  (1) his OWN post while it is being lost - the PS2 draws nothing; the port draws the mirror of what the attacker
  sees (his colour below, grey coming down at neutralize / NeutralizeTime) while the meter is above 0 or an enemy
  is inside; (2) which way the line moves - a small chevron at the line, up or down, from the sign of the meter's
  change this tick (none while it holds or is contested). The PS2's own signs stay as they are: contested = the
  half-second blink, neutral = the icon all grey.
- **The port's capture machine differs from BF2's in one place** (`core/src/cp.rs`: the leftover neutralize meter
  is zeroed at a capture); the icon reads the port's meters, so nothing to do.
- **Black box**: per post the running meter as a fraction, its team, the direction and "contested" (the units
  inside per team go into the snapshot's post row).
- **Acceptance the kit already serves**: the PS2's rows as a fixture - the function fed each row's post state must
  give the row's drawn flag, split (within 0.01, one update behind) and two colours (the alternating one as either
  of its pair); and the port's frames beside `sheets/*.png` with the symbol's box within 2 px of x 533..603,
  y 212..272 at 640 x 480.
- [M] **The rule of 3.1 replayed on the PS2's own rows** (`tools/replay.py`, a reference implementation with no
  fitted parameter, over `fixture/capture_display_ps2.csv`: 4,338 rows of 8 scenarios, 3 of BF1 and 5 of BF2, made
  by `tools/mkfixture.py`): 4,268 rows give the object's drawn flag, split and both colours outright, 64 more on the
  previous update's post state (the object is one update behind), 3 sit at a change of the post's state and 3 are
  two updates behind one - 4,332 of 4,338.
