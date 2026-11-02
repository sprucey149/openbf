# OPUS RE (2026-10-20) - [O] UNAUDITED
# RE: the start-of-level and respawn flow on the PS2 (BF2): side, post, class, the death camera (card SSEL)

Card SSEL asked: what the PS2's BF2 screens are at the start of a level and after a death, in which order, what each one
shows, which inputs move it, what the side-select options are, what the death camera does, and the captures that show it.
The sources are the BF2 ELF (`analysis/SWBFRNT2.ELF-decomp.c`, cited `B2:<line>`), the disc's compiled in-game Lua
(`extracted/bf2/lua/ingame/`), and live PCSX2 runs sampled once per game update (`engine/artifacts/lead_tmp/ssel/`, §12).
Tags: [V] read (lines cited), [L] measured live on the PS2 (file cited), [D] derived, [H] hypothesis. Load-bearing claims
are `[O]`. The verifier's verdicts are in §10. Offline means the PS2's "network byte" is 0 (`DAT_00600548`, or
`DAT_00600549` when `DAT_0060054f` is set; 0 in every state of this card); online-only branches are named, not built.

the PS2 flow - `engine/artifacts/lead_tmp/ssel/ps2_flow_sheet.png`
(BF2 Hoth, Instant Action, Conquest: the side select with each button highlighted, the spawn map and the class screen at
the level start, the spawn, the death camera, the respawn screens, the pause menu's Respawn).

## 0. The answers [O]

1. **Order at the start of a level: side select -> spawn map (post) -> class select -> spawn.** One state machine per
   viewport, `FUN_00398000`, state `SD+0x18`: 0 side, 2 spawn map, 3 class, 5 class accepted (then 2 with the display
   closed while the player lives); 4 is a class select opened while alive (§7). The side select shows only where sides
   may be chosen: Instant Action sets "can switch sides" (`DAT_00600b4e`), the campaign and Galactic Conquest clear it
   and their display opens straight on the spawn map (`B2:392469-392471`, `FUN_00426e80`). [V][L]
2. **After a death: the death camera for 4.0 s, then spawn map -> class -> spawn.** No side select again: closing the
   display leaves the state at 2 offline (`B2:392616`). The post and the class are the ones last used (the spawn map
   re-opens on the member's chosen post if it is still the team's). [V][L]
3. **Side select ("Select a team"):** a Lua button list (`ifs_sideselect`): team 1's name, team 2's name, **Auto Assign**;
   Spectate exists but is hidden offline. Up / Down move, X selects, no Back. The level behind it is the mission's first
   camera shot, with the HIGHLIGHTED team's first class standing in it as a 3-D preview; none on Auto Assign. [V][L]
4. **Auto Assign offline keeps the player's current team, which at the level start is team 1.** Instant Action calls no
   team setter for it; the spawn map then shows the member's own team (team 1 on 3 of 3 maps tried, and team 1 in all 37
   side-select states on file). The PS2's random coin flip (`FUN_0036fc40`) is reached only in Galactic Conquest when
   the campaign side is not one of the two teams; online it picks the team with fewer humans. [V][L] (the absence part
   checked by two verifiers, §10)
5. **Spawn map:** card SPSEL's directional post pick (already in the sim), the map view (card BMAP2), "COMMAND POST /
   CHANGE COMMAND POST / SELECT COMMAND POST" legend, the objective line. X accepts only with a post selected; Triangle
   does nothing offline. [V][L]
6. **Class select:** the team's classes as a line-up of 3-D soldiers in front of the camera shot, the selected one in
   front; a panel: the class name, "Unit Count: N" (or "Unit Count: N  Max: M"), "Equipment:" and the weapon names; a
   class that cannot be taken says "Get N more points to unlock!" or "Too many units already!" and X on it only plays the
   back sound. Left / Right (D-pad or stick beyond 0.8) cycle with wrap (Left = next, Right = previous); X "Select
   Character" spawns; Triangle "Back" returns to the spawn map. [V][L]
7. **The spawn is immediate.** Offline the human waits for no spawn wave: the member update skips the wave tests for a
   human, and the human is made ready to spawn only by the class accept. Measured: the soldier exists 0.10-0.13 s (3-4
   updates) after X on all three maps. [V][L]
8. **The death camera** is its own camera object (vtable 0x617d78, update `FUN_001bc188`, §6.5) [V][L]. **Phase 1**
   (every death): it eases in 0.6 s from the player's view to a view of the body from 6.22 m: the line from the body's
   point (feet + 0.8 m) through a point fixed at the death (1 m behind along the last camera facing, 1.5 m up), 5 m past
   it - for a body that stays put: about 5.2 m behind and 4.1 m above the feet, pitch about -32 deg. A body thrown past
   the fixed point swings the view around it (10 to 177 deg measured). **Phase 2** (an enemy kill only, from 1.5 s
   after the camera starts): it moves onto the body (about 1 m from the body point toward the killer) and looks at the
   killer's unit or vehicle (his feet + 1.70 m), following him live until the spawn map. A suicide (no killer stored)
   or a hero class (no killer passed) holds phase 1 for the whole 4 s. The camera sets no base field of view (only the
   zoom divisor 1.0): the screen keeps 65 deg, or 55.39 deg (tan_v 0.40237) when the player camera ran one more update
   on the corpse. The minimap and the kill line ("SF 821 KILLED PLAYER 1"; "PLAYER 1 DIED" after a suicide) stay, no
   reticle. 4.07 s after the death the spawn map opens over phase 1 again, dimmed to about
   half; at a respawn the camera shots are not used. [V][L] (10 enemy kills on Kamino, 16 AI a side, phase 2 in each
   one with rows; 3 pause-menu suicides and 1 explosion kill without; §6.5)
9. **Backdrops:** at the level start each screen change (side -> map -> class) cuts to another of the mission's
   `AddCameraShot` views (48 deg horizontal), picked at random but never the current one (§1.1). [V][L]

## 1. The state machine [V][O]

`FUN_00398000` (header `B2:392765`) runs each viewport's spawn display `SD` (`FUN_00395878`: `SD = *(0x6a8e80 + 4 *
viewport)`). Fields: `+0x18` state, `+0x1c` the human member, `+0x24` active, `+0x26` display up, `+0x58` spectate,
`+0x59` had a unit last update, `+0x5c..+0x7c` the five sound slots (`analysis/re-ambience-music.md` §6.3), `+0x88`
team, `+0x8c` selected post, `+0x98` class. The member (stride 0x1a4, pool `DAT_00661330`): `+0x130` player index
(>= 0 for a human; the human member reads 0 in every state here), `+0x134` team, `+0x13c` class, `+0x144` chosen post,
`+0x148` unit, `+0x158` respawn timer, `+0x164` ready to spawn.

| state | screen (Lua) | X (`+0x68 >= 0.125`, `B2:393041`) | Triangle (`+0x6c >= 0.125`, X up) |
|---|---|---|---|
| 0 | side select `ifs_sideselect%i` (`B2:393226`; pushed when not already the current screen) | -> 2, sound `+0x74`, `FUN_003ecb98` sets the team (`B2:393050-393054`) | nothing (the Lua screen has no Back) |
| 2 | spawn map `ifs_mapselect%i` (`B2:393290`) | only with a post selected (`SD+0x8c != 0`): -> 3, sound `+0x74` (`B2:393056-393061`) | offline nothing; online -> 0 when `DAT_00600504 == 0` (`B2:393160-393167`) |
| 3 | class select (`FUN_00399dd8`, `B2:393025-393027`) | class refused (`FUN_00364290`): sound `+0x7c` only; else `FUN_0034dac0`, -> 5, sound `+0x64`, spawn request `FUN_0039b748` (`B2:393063-393132`) | -> 2, sound `+0x7c` (`B2:393171-393173`) |
| 4 | class select while alive (§7) | as state 3 | -> 5, sound `+0x7c` (`B2:393175-393177`) |
| 5 | class accepted | - | -> 3, sound `+0x7c` (`B2:393180-393183`) |

- [V] A unit lost (`SD+0x59` set, the unit flag now 0) sets state 2 (`B2:392895-392899`); closing the display
  (`FUN_00397840`) sets 2 (`B2:392616`; 0 only online). So after a death the display can only re-open on the spawn map.
- [V] Outside state 4, each update decides open / close: the display opens (`FUN_00396a00`, `B2:392946`) when the
  member (the pool entry of the viewport's player, `FUN_003d6db8(SD+0x2c)`) has no unit, its timer `+0x158 <= 0`
  (`B2:392942`), spectate is off (`B2:392943`), `FUN_004241a0()` is 0, it is not already up, and its team has tickets
  (`team+0x24 > 0`, `B2:392963`); otherwise an open display is closed (`FUN_00397840`) - with no tickets: no respawn.
- [V] Opening in state 0 offline jumps to 2 when `FUN_00426e80()` is 0 (`B2:392469-392471`): offline that function is
  `DAT_00600b4e && !DAT_00600b4c` (`B2:483046` the only writer [receipt:d8591d07], `FUN_00426ee0`; called with 1 by rule set 0
  "instantaction" `B2:43346`, with 0 by "campaign" and "metagame" `B2:43354`, `43359`, and by
  `ScriptCB_SetCanSwitchSides` `B2:43292`). RAM: 1 in the IA states. So the side select is an Instant Action screen.
- [V] Opening copies the member into the display: team (`B2:392291`), the member's chosen post if its owner object is
  still valid, else the team's default post `FUN_0036cca0` (`B2:392300-392312`), the member's class (a hero class is
  replaced by 0; where heroes are on offer, `FUN_0039a888`, the first hero class - `B2:392315-392358`); and picks a new
  camera shot (`FUN_001b9d00`, `B2:392548-392553`, §1.1).
- [L] Every screen change happens on the update after the key (`t7_*/rows.csv`: X at 186.1615 -> state 2 at 186.2616).
- [V] The whole input block is skipped while `…
  X and Triangle are edge-latched (`SD+0x30`, `SD+0x31`).

### 1.1 The backdrop: the mission's camera shots [V][L]

- [V] `AddCameraShot(qw, qx, qy, qz, x, y, z)` (header `B2:31892`) appends a rotation (+0xa0..) and a position
  (+0x1a0..) to the camera manager `DAT_005fc4ec` while it holds fewer than 16 (`B2:31916`; the quaternion is stored
  in the order x, y, z, w). Every BF2 conquest mission calls it 3 to 22 times (33 of 33 `*_con` scripts; those with 21
  or 22 keep their first 16); BF1's mission scripts call it too (44 of 44 files [receipt:b3866474]).
- [V] `FUN_001b9d00` (`B2:63214`) picks the next shot for a viewport: with n shots, n < 3 -> (current + 1) % n
  (`B2:63243`); n >= 3 -> an LCG draw: `u = s * 0x19660d + 0x3c6ef35f; s = u * 0x19660d + 0x3c6ef35f;
  index = ((u >> 16) | (s & 0xffff0000)) % n` (`B2:63245-63248`), drawn again while it equals the current shot (at
  most 16 times), then +1 if still equal. Called by every `FUN_00396a00` (each opening and each screen change) unless
  `SD+0x27` is set.
- [L] Level start, Hoth: the camera's eye at the side select, the spawn map and the class screen equals hot1g_con's
  shots 0, 1, 2 exactly (`t1_hot1/s0_team1`, `s3_after_auto`, `s4_class`; `pose.py`), at tan_v 0.34131 (the default
  48 deg horizontal). Kamino and Naboo: a new eye on each screen change (`t7_*/rows.csv`).
- [L] At a respawn the shot index still changes per screen but the eye stays the death camera's (§6, item 3).

## 2. Side select [V][L][O]

- **The screen** [V] `ifs_sideselect.luac` proto 0 (listing `lead_tmp/ssel/dis_ifs_sideselect.txt`): a vertical button
  list tagged `team1`, `team2`, `auto` ("ifs.sideselect.autoassign"), `spec` ("ifs.sideselect.spectate"); title
  "ifs.sideselect.chooseteam" ("Select a team"), `bBlackBackdrop`, `bDimBackdrop`, `bAcceptIsSelect`, no Back help text;
  Back / Left / Right do nothing. Four screen tables (`ifs_sideselect1..4`, one per viewport in split screen); copies 2-4
  carry Up / Down handlers following `link_up` / `link_down` (`FollowButtonLink`, which plays `shell_select_change`,
  re-ambience-music §6.4); copy 1 has none of its own (the shell screen's default moves the cursor; live, Down did).
  The team buttons' layout string is a placeholder ("ifs.sideselect.spectate") the engine overwrites with the team name.
- **Which buttons** [V] `ifs_sideselect_fnEnter` (proto 0.1) hides a button for 0, dims it for 1 and shows it otherwise,
  from `ScriptCB_GetSideSelectButtonSetting` (`B2:43971-44003`): offline team 1 = 2, team 2 = 2, auto = 2 (`B2:44001`),
  spectate = 0 (`B2:43997`). Online the team buttons dim when `DAT_00600504` is set and Spectate shows.
- **Labels** [V] `FUN_003ecd90`: each team button shows the team's name (team object `+8`, e.g. "Empire" / "Rebels",
  "Rebels" / "Empire" on COR1); online "%s   (%d)" with the human count (`B2:447240`, locl `0x5071dc6b`).
- **The preview** [V][L] every update of state 0 (`B2:393254-393270`, gated by `DAT_005ff825`, ELF default 1, no writer [receipt:5b2d36d0])
  the engine reads the Lua `CurButton` (`FUN_003ecaa0`, string `0x633680` "CurButton", tags at `0x633690..0x6336a8`) and
  for the highlighted team sets `SD+0x88` and shows that team's first-class soldier (`DAT_00825ea0`, built by
  `FUN_00395a68`); on Auto Assign no soldier. Hoth (`ps2_flow_sheet.png` row 1): Empire -> a Snowtrooper, Rebels -> a
  Rebel soldier, Auto Assign -> empty set. RAM (`t1_hot1/s0..s2.p2s`, `t7_*/rows.csv`): `SD+0x88` = 1, 2, 2 as the cursor
  moves down.
- **Accept** [V] `FUN_003ecb98` (`B2:447095`): team1 / team2 -> `FUN_0039b828(SD, team)` (`B2:447166-447169`), which
  sets the member's team (`FUN_0034f100`) and the default post and class; spec -> spectate, state 5 (`B2:447151-447153`).
- **Auto Assign** [V][L][O]: with `DAT_00600b4c == 0` and offline it returns with no team call [receipt:0400519d] (`B2:447114-447127`; ELF
  `0x3ecc60` loads `DAT_00600b4c`, `0x3ecce8` the network byte, branch to the epilogue `0x3ecd7c`). The next update of
  state 2 sets `SD+0x88` to the member's team (`B2:393289`): the player's team stays as it was. `DAT_00600b4c` is 1
  only under rule set 3 (`FUN_0019bfc8`, `B2:43357`), and `ScriptCB_SetGameRules` (`B2:43400`) numbers its argument among
  "instantaction", "mp", "campaign", "metagame" (pointers at `0x5fa7c0`): only Galactic Conquest uses the campaign's side
  (`DAT_00825220`, written only by `ScriptCB_MetagameSetSide` [receipt:15b1a896]) and, failing that, `FUN_0036fc40` (its one caller, one jal word [receipt:b49472ca]): offline a coin flip `((LCG >> 16) & 1)
  + 1` (`B2:368168`), online the team with fewer humans (`FUN_0036fb28`, `B2:368144`; 1 when `DAT_00600505`). Live, offline Instant Action
  (rule set 0 in RAM): Auto Assign -> team 1 on Hoth (Empire; `t1_hot1/s2_auto` -> `s3_after_auto`), Kamino CW and
  Naboo CW (`t7_*/rows.csv`: `sd_team` 1 -> 2 -> 2 -> 1, `m_team` 1 throughout); the human member's team is 1 in all 37
  `ref/states/bf2_*sideselect*.p2s` (17 maps, `lead_tmp/ssel/sdstate.py`). A coin flip would give 3 of 3 with p = 1/8.

## 3. Spawn map [V][L]

- The pick is card SPSEL's (`analysis/re-command-walker-post.md` §5, `FUN_0039a058`): D-pad / stick by direction over
  the team's posts, `along + 0.25 perp^2`. The map element is card BMAP2's Spawn view (`analysis/re-hud-map.md`).
- [L] What it shows (`ps2_flow_sheet.png` row 2 left): the map disc on the left, numbered posts in team colours, the
  selected post's ring, the north arrow, top right the mission's objective ("CAPTURE ALL THE COMMAND POSTS"), bottom left
  three legend lines: the post icon "COMMAND POST", the D-pad "CHANGE COMMAND POST", X "SELECT COMMAND POST". No Back line.
- [V] X needs a selected post (`B2:393057`); Triangle does nothing offline (`B2:393160-393166`). The team shown is the
  member's (`B2:393289`).
- [V] After a side accept the default post is set with the team (`FUN_0039b828` -> `FUN_0039b920` -> keep-or-nearest
  `FUN_0036ca08`); on Hoth it was the AT-AT's post (`t1_hot1/s3_after_auto.p2s`, `SD+0x8c` = the walker post SPSEL saw).

## 4. Class select [V][L][O]

- **Input** [V] `FUN_00399dd8` (`B2:393625`): left stick X, or the D-pad (`+0x88` -> -1, `+0x8c` -> +1; ELF immediates
  0.125, -1.0, 1.0); fires past x^2 > 0.64, re-arms under 0.04 (`B2:393672-393680`), not while the line-up slides
  (`SD+0x33`); x < 0 -> class + 1 wrapping to 0, x > 0 -> class - 1 wrapping to the last (`B2:393690-393705`), sound
  `+0x5c`. [L] Right took class 0 -> 5 and Left 5 -> 0 on a 6-class team (`t7_*/rows.csv`). Up / Down do nothing (UAUD2).
- **The panel** [V] `FUN_00396430` (`B2:392007-392110`), strings from the disc's English table: the title bar = the
  class's name; "Unit Count: %d" (or "Unit Count: %d  Max: %d" when the class has an unlock cost and a max,
  `B2:392051-392057`); "Equipment:" (`B2:392061`) and one line per weapon; a class the game refuses (`FUN_00364290`)
  first says "Get %d more points to unlock!" (`class+0x374` minus the player's points, `B2:392041-392045`) or "Too many
  units already!" (`B2:392037`). [L] `ps2_flow_sheet.png`, `lead_tmp/unit-audit/pcsx2_bf2/cor1g_sheet.png` (locked
  classes drawn as black silhouettes, "Get 12 more points to unlock! Unit Count: 1 Max: 4").
- **The line-up** [V][L] per team, every class's soldier model (`FUN_00395a68`, `DAT_006a5b30` per viewport x team x
  class, up to 12) stands in the camera shot; the selected one in front, slid in by the carousel; help line "Back"
  (Triangle) left, "Select Character" (X) right.
- **Accept** [V]: refused -> back sound, stay (re-ambience-music §6.3); else the class is set and the spawn requested.

### 4.1 The line-up's ring (OPUS RE 2026-10-20, card SSELB slice 4) - [O] UNAUDITED

Hops: `FUN_0039a980` (the class screen's update) -> `FUN_0039a5f8` (one soldier's place) -> the tables `FUN_0039bcb0`
writes; `FUN_00394f98` / `FUN_00395a68` (the containers and models). Checks run: `re_check fn` on every cited line,
`re_check word` 0x5ff828 (150.0, 80.0, 0.0077, 0.0154); one opus verifier (C1-C7 SUPPORTED, C8 PARTLY on wording, fixed
below; it read `DAT_00600b30`'s ELF word 1 and its writers: the splitscreen count).
- [V][O] Six places on a unit ring, `DAT_006a5080` (x, 0, z) (`FUN_0039bcb0`, B2:394865-394895): (-0.6659, -0.9588),
  (-0.7071, 0.7071), (0.5588, 3.0), (0.9659, 0.2588), (0.5071, -0.7071), (-0.2588, -0.9659); colours `DAT_006a50e0`
  b4 b4 b4 ff for places 0-4, 0 0 0 1 (unseen) for place 5 (B2:394881-394917).
- [V][O] `FUN_0039a5f8(t, sd, model, place, selected)` (B2:393979-394046): position = lerp(place, place + 1 mod 6, t)
  x 150 (`DAT_005ff828`) + (100, 350, -452) (`DAT_006a5050`) with one viewport (`DAT_00600b30` = 1, its ELF default;
  split screen: x 80 + (100, 15, -180)); the colour word the same lerp of the colours; `selected` to the model's
  vtable +0x54.
- [V][O] Still (`SD+0x33` = 0, B2:394201-394218): with c = `SD+0x98` and n classes, n >= 5: c-2, c-1, c, c+1, c+2 at
  places 0-4 (c selected); n = 4: c-1..c+2 at 1-4; n = 3: c-1..c+1 at 1-3; n = 2: c, c+1 at 2, 3; n = 1: c at 2.
- [V][O] A step (`FUN_00399dd8`, B2:393672-393691) sets `SD+0x33` = 1, `+0x3c` = 0, `+0x34` = old c, `+0x38` = new c,
  `+0x40` = 0 for c + 1 (stick left), 1 for c - 1; a step while a slide runs is dropped. The slide: `+0x3c += dt x 3`,
  done at 1 (B2:394130-394136): **a third of a second**. With `+0x40` = 0 each soldier goes from place k + 1 to k
  (t = 1 - s; the ring turns back); for 2 <= n <= 5 the one that wraps sweeps the back places,
  f = (2 - (n-1)/2) - s (7 - n) (B2:394236-394283); `+0x40` = 1 is the mirror.
- [V] The line-up's container stands at (-`DAT_007d5f70` / 2, 18 - `DAT_007d5f74` / 2) (`FUN_00394f98`,
  B2:391575-391581); every screen soldier is built by `FUN_00395898` with `FUN_005a3368(model, 0xbe)` (B2:391745) and
  the scale 0.0077 x 0.4.
- [H] `DAT_007d5f70` / `74` are the 512 x 448 screen (the front soldier's x fits it).
- [M] The UI scene's perspective, fitted to `lead_tmp/ssel/t1_hot1/s4_class.png` (Hoth, Empire, 6 classes: the dark
  trooper at place 1, feet at y 334, x about 131; the officer at place 0; the Snowtrooper in front, x 230): the eye 500
  units before the screen plane, 1.25 px across and 1.507 px down per unit of the 640 x 480 frame, the far vanishing
  point at y 201. The port maps the scene into the camera's frame with it (`game/src/spawn_preview.rs` `lineup_at`).
- [V] (card SSELB slice 9) The arm `SD+0x25` is set only by a step that FIRED (B2:393674-393678) and cleared when the
  push falls under x^2 0.04 (B2:393680-393682); a push blocked by the slide leaves it clear, so a push HELD through a
  slide steps again in the update after the slide ends. The input runs first and then the slide's advance in the same
  update (`FUN_00398000` B2:393025-393027 calls `FUN_00399dd8`, which ends in `FUN_0039a980`, B2:393714; jal words at
  0x3985f4 / 0x39a01c, the only calls). [D] BF2's step is 0x3d088888, dt x 3 = 0x3dcccccc (separate mul.s / add.s,
  0x39a9fc / 0x39aa08, then c.le.s 1.0), and the EE's add rounds toward zero: ten adds read 0x3f7ffffd (< 1), so the
  slide ends in its 11th update counting the step's own and a held push repeats every 11 updates (PROVISIONAL until a
  PS2 row pins the boundary; the verifier's catch - IEEE nearest would give 10). The accept (X, B2:393040-393075) has no slide test. The arm is
  shared with the map's push (`FUN_0039a058`, B2:393792-393969) and set when the display opens (`FUN_00396a00`,
  B2:392362). Built: `Sim::class_carousel` (`core/src/sim/spawn/runtime.rs`).
- Not built: the selected flag's pose (vtable +0x54), the third vector `DAT_006a5070` (25, 0, -25).

### 4.2 The screen soldiers' light (OPUS RE 2026-10-21, card SSELB slice 7) - [O] UNAUDITED

Hops: the UI scene's draw walk `FUN_005a3d40` -> the UI model `FUN_0037f110` / the soldier `FUN_0037fd30` -> the model
renderer's slot +0xc `FUN_0057cf60` (and `FUN_001de2a8`) -> the light set `0x7d6160` -> its writer on the spawn screens
`FUN_00398000`. Checks run: `re_check fn` on every cited line; `writers` / grep of every global below; the ELF words for
the vtable slots (0x62d9b8 = 0x37f110, 0x64febc = 0x57cf60, 0x62f2b4 = 0x398000); `absent b2 0x6a4ff0 0x6a5008
0x6a5020 0x6a5038 --elf` [receipt:49141124] (no reference outside `FUN_00398000`); one opus verifier (C1-C4, C6-C9
SUPPORTED, C5 PARTLY: see the [M] line below).
- [V][O] The draw walk hands each element its colour word (+0x40..+0x43) times the UI colour `DAT_0076c200` / 255
  (B2:738032-738046, `FUN_005a3d40`); `DAT_0076c200` is white, written only by `FUN_0057ca38` (B2:712404-712427).
- [V][O] Every UI model draw passes the flag 0x4000000: the model `FUN_0037f110` (B2:378477), the soldier
  `FUN_0037fd30` (body B2:379314 / 379336, weapons B2:379302 / 379307, attachments B2:379352). The renderer's slot +0xc
  (vtable 0x64feb0, `FUN_0057cd68` B2:712517; ELF word 0x64febc = `FUN_0057cf60`) then lights the model with the fixed
  set **0x7d6160** instead of the lights gathered at its place (`FUN_00597e98`): B2:712762-712787; `FUN_001de2a8` the
  same, B2:90223-90227. So a UI model never takes the map's sun, ambient or local lights.
- [V][O] 0x7d6160 is the INTERFACE light set (`FUN_005a38b8`, B2:737633-737647: two lights 0x7d5fa0 / 0x7d6080, the
  hemisphere colours at +0x50 / +0x60, their mean at +0x1c); Lua's `ScriptCB_SetIFaceLighting` and
  `ScriptCB_SetSunlight` write it too (B2:29244-29273, B2:28890-28897), and `FUN_00386480` saves, sets and restores it
  around its own draw (B2:382784-382852).
- [V][O] On the spawn screens `FUN_00398000` (slot +0xc of the spawn display's vtable 0x62f2a8, `FUN_00394c50`
  B2:391376) sets it on EVERY call, before its first return (B2:392811-392851): light 0's travel direction
  (-0.57735, -0.57735, -0.57735), colour (1, 1, 1, 1) (packed x 128 by `FUN_00595a78`: GS 1.0); both hemisphere colours
  (0.5, 0.5, 0.5, 1) and their mean the same - a flat ambient of 0.5. The constants `DAT_006a4ff0..6a5044` have no
  other writer (once-guards in the function itself).
- [V][O] Colour words: the side preview 0xffa0a0a0 (B2:391928), the line-up the lerp of the place words 0xb4 grey
  (`FUN_0039a5f8`); a class the game refuses (`FUN_00364290` == 0) gets `DAT_0076c220` = (0, 0, 0, 0xff) instead
  (`FUN_0039a980`, B2:394368-394383): a black silhouette (the Hoth frame's dark trooper and officer, torso median 6).
- [M] Light 0 is ON at draw time (the verifier: `FUN_005a45d0`'s light constructor runs after `FUN_005a38b8` and
  zeroes the enable bit, and `FUN_005a38b8` has two more callers, so the code alone does not settle it): rendered with
  the ambient alone the side Snowtrooper's p50 is 31 against the PS2's 61; with the light, 67.
- [M] The direction lies on the port camera's frame as written (x right, y up, z toward the viewer: the light comes
  from the upper right, in front): of the eight signs this one scores the side Snowtrooper best (mean |lum| error over
  his pixels 10.3; the others 19-36 at the same colour scale), brighter on his screen right as the PS2's (62 / 71
  against 57 / 66). The colour word acts as byte / 255 on the light: byte / 128 clips him white under every sign
  (p50 105-121 against 61). Side p50/95/99 port 67/97/109, PS2 61/93/102 (before: the map's lights, 65/123/130).
  `lead_tmp/sselb/tools/light_score.py`, renders in `lead_tmp/sselb/s7/`.
- Port: `render::LightPath::Iface` (`IFACE_AMBIENT`, `IFACE_DIR_CAMERA`), `game/src/spawn_preview.rs`. The refused classes'
  black is built (§4.3): on a BF1 map the same black marks the specials the port locks (`game/src/unlocks.rs`, card
  UNLK1), though BF1 itself has no unlocks.

### 4.3 Which class the game refuses (OPUS RE 2026-10-21, card SSELB slice 8) - [O] UNAUDITED

Hops: `FUN_00364290` (the refusal test) -> its four callers -> the cost's setter `FUN_0026a340` and the max's
`AddUnitClass`. Checks run: `re_check fn` on every cited line; `callers` + the ELF's `jal` words (0x34e544, 0x3964f8,
0x3986d0, 0x39b3c8: exactly four); `absent b2 FUN_00364290 0x364290 --elf` [receipt:2f92f869]; `word` 0x26a4e0 (lui
0x714b / ori 0x2463) and 0x61fc20 ("%d"); `writers DAT_00661334`. One opus verifier: C1-C6 and C8-C12 SUPPORTED, C4 /
C7 / C12 narrowed (folded in below).
- [V][O] `FUN_00364290(stats, class, out)` (B2:360335-360391) refuses (returns 0) when the member's team or the class is
  missing (out = 1); when the points `FUN_00361900(stats, 1)` = the s16 at stats+0x1fc (the non-hero points,
  B2:358435-358442) are below the class's cost `+0x374` and the cost is not 0 (out = 1, "Get N more points"); and, for a
  cost >= 1 only, when the team's members with a live unit of that class (`FUN_0036f538`) are at or over its max
  (`FUN_0036f520` = team+0x54[i]) (out = 0, "Too many units already!"). A class with no cost has no max test.
- [V][O] The cost is the ODF's `PointsToUnlock` (hash 0x714b2463, `FUN_0026a340` B2:186907-186909, "%d"); the max is
  `AddUnitClass`'s fourth argument / the `SetupTeams` slot's third (default 0x7fffffff, B2:32651-32695 ->
  `FUN_0036fd98` writes +0x50 min / +0x54 max). On the BF2 disc: every officer 8, the dark trooper, wookiee, jet
  trooper (and the CIS special) 12, the `_hunt` / `_training` variants 0 (the baked `classes/bf2_*.ron`).
- [V][O] The last test (B2:360378-360383) is an IsKindOf on the droideka class type (`DAT_00689308`,
  "EntityDroidekaClass"): a non-droideka class passes; a droideka class needs a free slot in the "EntityDroideka"
  memory pool (+0x34 capacity, +0x3c [H] live count), else out = 0.
- [V][O] Four callers only [receipt:2f92f869]: the panel `FUN_00396430` (B2:392034), the accept `FUN_00398000`
  (B2:393072: refused -> the sound +0x7c and stay), the line-up colour `FUN_0039a980` (B2:394375: refused -> black) and
  the member respawn `FUN_0034e1f0` (B2:345006), which checks only a HUMAN member (+0x130 >= 0) whose class index is not
  0: a refused one goes back to class 0 (`FUN_0034f2c0`) and the screen (`FUN_0034ffe0`). **No bot is refused**: AI
  members skip the check and the AI pick (`FUN_0036f198`, via `FUN_0034ffe0`) reads min / max only (max - 1 with a human
  on the team or under 60 s on the clock), never the cost.
- [V][O] The panel (B2:392035-392058): the refusal line first (0xd3cd201e, or 0x155ab074 with cost - points), then
  "Unit Count" 0xecf8496f with the count alone when max > `DAT_00661334` (every member record allocated, all teams:
  +1 per member in `FUN_0036e3e0`) or the cost < 1, else 0xcb22a09f with count and max.
- Port (slice 8): the human's refusal and the panel lines, the black line-up. Not built: the droideka pool test (no
  playable droideka class in a BF2 line-up); the refused human's fall back to class 0 at a respawn (the port's screens
  always ask).

## 5. The spawn [V][L][O]

- [V] `FUN_0039b748` offline: `FUN_0034f590(member, team, class, post)` then `FUN_0034d9a0(member, 1)` (`B2:394509-394510`),
  which writes the member's ready byte `+0x164` (`B2:344395`).
- [V] The member update `FUN_0034cea8`: the timer counts down and the update returns while it is above 0
  (`B2:343997-343998`); the spawn needs the ready byte (`B2:344138`); while it is 0, `FUN_0034ffe0` runs (`B2:344001`),
  which for a human only clears it and returns (`B2:346288-346290`) and for an AI picks a class and a post and readies it
  (`B2:346291-346299`). Offline a human (`+0x130 >= 0`) takes the team pointer (`B2:344178-344182`) and skips the two wave
  tests (`B2:344186-344192`: the team's wave counter against the member's wave, the release interval) that an AI member
  passes. **The human never waits for a spawn wave, and never spawns without accepting a class**: of the 11 calls of
  `FUN_0034d9a0` (11 jal words) only `FUN_0034ffe0`'s AI branch, `FUN_0039b748` and the network handler `FUN_003f1670`
  pass 1 [receipt:fcbf78d9].
- [L] X on the class -> the soldier 0.10-0.13 s later: Kamino 206.0147 -> 206.1148, Naboo 199.9481 -> 200.0816 (4
  updates), Hoth 227.1812 -> 227.2813 (`t7_*/rows.csv`, `t2_hot1_death/rows.csv`); state 5 for one update, then 2 with
  the display down.
  The camera eases from the shot's 48 deg to the soldier's 65 deg (tan_v 0.34131 -> 0.42154 -> 0.4587 -> 0.48838: the
  zoom doc's exp(-25 dt) FOV ease, `analysis/re-zoom.md` §5).

## 6. After a death [V][L][O]

1. [V] A death (`FUN_0034f650`) sets the member's timer to 4.0 (`B2:345913`) and clears its unit (`B2:346045`); a class
   flagged in bit 4 of the unit's `+0x228` word gets 1e-4 instead (`B2:346055`, re-tickets-spawn §1.5).
2. [L] **The death camera** (`t6_hot1_suicide`, `t8_kam1_suicide`: a suicide through the pause menu, one row per update):

   | after the death | eye behind the feet | eye above the feet | pitch | yaw |
   |---|---|---|---|---|
   | 0 (alive) | 3.10 m | 2.57 m | -14.0 | 97.3 (Hoth) / 264.9 (Kamino) |
   | +0.13 s | 3.72 | 2.98 | -19.5 | unchanged |
   | +0.27 s (Kamino) | 4.68 | 3.64 | -27.1 | unchanged |
   | +0.43 s | 5.14 | 4.08 | -32.1 | unchanged |
   | +0.6 s .. +4.0 s | 5.25 (Kamino) / 5.19 -> 5.11 (Hoth) | 4.05 / 4.13 -> 4.20 | -31.8 / -32.6 -> -33.6 | +0.0 / +0.4 deg in 4 s |

   It looks at the body along the player's last facing (the direction from the body to the eye is the camera's own yaw
   to 0.1 deg); the yaw moved 0.4 deg (Hoth, `t6`), 0.0 deg (Kamino, `t8`) and 6.7 deg (Hoth, `t2`, a body that slid)
   in the 4 s. FOV unchanged (tan_v 0.48838 = 65 deg horizontal, zoom 1.0). The soldier's state reads 15 (dying). On
   screen (`ps2_flow_sheet.png` row 3, `t2_hot1_death/death_sheet.png`): "PLAYER 1 DIED" top left (card EVMSG), the minimap
   stays, no reticle, ammo or health. [V] The soldier camera's dead branch (`FUN_0024cd00`: the alive bit E+0x228 bit 3
   clear) keeps using the stance's camera section with a 0.1 factor where a living soldier passes 1.0
   (`B2:166906-166910`, `FUN_0024cd00`: `0x3dcccccd`, replaced by `0x3f800000` when alive, passed to `FUN_001cef68`; the
   same at `B2:166891-166895` for `FUN_001cf738`) - but the view after a death is NOT the soldier camera: the death
   camera of §6.5 takes over (second pass), and its code gives the pull-back's geometry and timing.
3. [L] **4.07 s after the death the spawn map opens** (Hoth 246.3672 -> 250.438, Kamino 223.5658 -> 227.6365): two
   updates after the timer first reads 0 or less (the camera snaps one update before the screen shows). The level behind it stays the death camera's (the eye snaps back to 3.1 m / 2.6 m
   and repeats the 0.6 s pull-back under the screen - phase 1 only, with the same turn, never phase 2; [D] the unit's
   destructor `FUN_001cdfc8` re-starts the death camera with no unit and no killer from the player camera's last
   matrix, `B2:77375`, §6.5), dimmed: the mean colour of an open patch of snow falls from about
   (194, 193, 198) to (98, 98, 100), ratio 0.51, within 0.3 s (`t6_hot1_suicide/strip_0.mkv`). The camera-shot index still
   changes per screen (`t2_hot1_death/rows.csv` "shot" 2 -> 1 -> 2) but the eye never takes a shot's position: at a
   respawn the backdrop is the place you died.
4. [V][L] The spawn map re-opens on the member's last post (if still the team's), the class screen on the last class
   (Hoth: the AT-AT post and the Snowtrooper again, `t2_hot1_death/d1_spawnmap.png`, `d2_class.png`); X, X -> the
   soldier 0.1 s later.

### 6.5 The death camera in code (second pass, 2026-10-20) [V][L][O]

- **Who starts it** [V]: the soldier update `FUN_00240bf8`, once the unit is dead (alive bit clear) and its player's
  member has let go of it (`member+0x148 == 0`), calls `FUN_001ce0d8(E+0x288, E+0x334 the player index, E+0x190)`
  (`B2:159446-159459`); a class with bit 33 of `class+0x330` set passes 0 instead of `E+0x190` (`B2:159456-159463`; §1
  reads that bit as the hero flag; the test is at `B2:159458`). `FUN_002379f0` does the same with `E+0x330`, skipped
  when bit 4 of `E+0x228` is set (`B2:151772-151780`). `FUN_001ce0d8` is a thunk to
  `FUN_001d0a48` (`B2:77402`), which, when the player index has a viewport (`FUN_003d6dd8 >= 0`), calls
  `… that one, only while the viewport's current camera is its player camera (`B2:63666`), initialises the
  viewport's death camera from the player camera's matrix and makes it current (`B2:63668`). Callers: `FUN_001ce0d8` 6
  jal words [receipt:602956ad], `FUN_001ba658` 1 [receipt:09081883], `FUN_001bc5f8` 2 [receipt:6b7e0f71] (the other is
  the unit destructor, below).
- **The killer** [V][L]: `E+0x190`. Live, on the two saved enemy kills it pointed into the member pool `DAT_00661330`
  (entries 20 and 29, team 2, AI), whose unit (`+0x148`) was the very soldier the phase-2 view pointed at (3-6 deg off)
  (`lead_tmp/ssel/t9_kam1_ai16_kill/k1_after.p2s`, `k2_after.p2s`); after a pause-menu suicide it is 0 (`t6`, `t8`
  states). [H] who writes it (the damage code; not traced).
- **Set-up** `FUN_001bc5f8(camera, unit, killer, matrix)` (`B2:65317`) [V]: unit at `+0xc` (`B2:65335`), the matrix
  copied to `+0x10..+0x4f` (eye at `+0x40`), timer `+0x70 = 0` (`B2:65360`), killer `+0x74 = 0` (`B2:65365`); then
  `+0x60` = the unit's position + (0, 0.8, 0) (`B2:65370-65375`, `0x3f4ccccd`), `+0x50` = the unit's position + (0, 1, 0)
  + (the matrix's third row made horizontal and unit, with y = 0.5) (`B2:65393-65405`); the killer is kept only when it
  differs from the word `+0xb8` of an object the unit hands back (`B2:65406-65414`; [H] the player's own record).
- **Update** `FUN_001bc188(dt, camera)` (`B2:65050`, only a vtable slot: data word 0x617d84 [receipt:59c7f5bd]) [V]:
  timer += dt (`B2:65115`); `+0x60` = the unit's position + 0.8 up, every update (`B2:65117-65119`). Then, with the
  ELF constants 1.0 / 1.5 / 6.0 at `0x5fc51c` / `0x5fc520` / `0x5fc524`:
  - timer < 1.0: weight t = timer / 1.0; from = `+0x50`, look at `+0x60` (`B2:65137-65145`);
  - timer >= 1.0: t = 1; from `+0x50`, look at `+0x60` (`B2:65148`) - unless timer > 1.5 and a killer is kept:
    t = min(1, (timer - 1.5) x 0.5) (`B2:65149-65151`), look at the killer's vehicle (`killer+0x14c`) or else his unit
    (`+0x148`) at its `+0xd0` (`B2:65150-65164`; live: 1.700 m above his feet, within 0.06 m across), from = `+0x60`
    (the body point). A killer with neither keeps the phase-1 points (with the phase-2 weight).
  - Two casts of 6.0 m from `from` +- 0.75 m up, backward along the view line (`FUN_00161c60`, `B2:65182-65211`);
    eye = from - dir x (6.0 x the smaller fraction - 1.0) (`B2:65218`): with nothing behind, 5 m behind `from`.
  - The camera then moves by t toward that pose: rotation slerped (`FUN_0053a3e8`, `B2:65272`), eye = eye + (target -
    eye) x t, stored (`B2:65285-65290`). Applied every update, so phase 1 converges in about 0.5 s and phase 2 in about 0.6 s.
  - Its "apply" slot `FUN_001bc5a0` (data word 0x617d88) only copies the matrix and sets the zoom divisor S = 1.0
    (`B2:65310-65311`, the pair the player camera uses with no entity, `B2:64543-64546`): it never sets the base field
    of view (the view's tangents), so the screen keeps the last base FOV set, unzoomed.
- **Measured against it** [L] (`t9_kam1_ai16_kill`, `t11_kam1_ai16_kill3`, `t12_kam1_ai16_aim`: Kamino, 16 AI a side,
  the player put beside the team-mate nearest an enemy at health 1 until killed, one row per update; `t6`, `t8`):
  - the hold distance from the feet as they were at the death + 0.8 m: 6.221 m (Hoth suicide), 6.219 m (Kamino
    suicide) against 1.2207 + 5 = 6.2207 m [D] (to the corpse's later feet, which settle 0.07-0.08 m higher: 6.14-6.18 m;
    [H] the point the camera reads is not exactly the soldier's `+0x150`); the ease, with the same anchor: the fraction left after n updates 0.967 / 0.812 / 0.360 / 0.047 against the product of
    (1 - k/30) with a one-update lag: 0.967 / 0.812 / 0.360 / 0.047 [D];
  - phase 2's swing begins on the first update after +1.5 s in all 7 enemy kills with rows (in 3 of them the eye was
    still drifting with the thrown body before that) and settles by +2.1..2.3 s, the
    eye 0.2-1.2 m from the body point; in a census 2.5 s after the death the view's yaw is within 0.1-4.0 deg of a
    living enemy in 7 of 8 enemy kills (`t10`, `t11`, `t12` `died_plus2_5` events; one with a single enemy within 30 m:
    3.4 deg); the eighth (`t11` death 0) was mid-swing at 57 deg up when the census ran; it follows the killer (that
    one's yaw swung 1.5 -> 152 -> 92 deg as he moved);
  - phase 1's turn: the unit's own facing (`E+0x140`) never changes after the death, but the body moves (0.3-1.8 m,
    thrown by the hit) while `+0x50` stays where it was set: 10 to 177 deg in the 7 enemy kills, 0 in the suicides;
  - one kill by an explosion that killed four team-mates (`t11` death 1, body moved 0.08 m): no phase 2, the view held
    phase 1 for 4 s; its FOV read 0.34131 (48 deg) before and after (the soldier was in state 9, [H] not a foot state);
  - FOV: tan_v 0.48838 -> 0.40237 on the update before the member lets go of the unit, in all 7 enemy kills; in the
    pause-menu suicides it stays 0.48838. [D] the player camera eases the FOV toward a dead soldier's default 48 deg
    (`analysis/re-zoom.md` §5) for one update before the switch: 48 + 17 x e^(-25/30) = 55.388 deg, tan(27.694 deg) x
    0.7666 = 0.40237; the respawn screens keep it, then the next soldier eases it back to 65 deg (0.44997, 0.47147, ...).
    This matches CFOVM's reported FEL1 tangent (tan_half_v 0.40237) [D]. Identifying that particular
    `cp3_cp3_1_postA_v3` state as a death or respawn screen remains [H]: its own JSON and save-state RAM must
    establish the active camera, human member/unit and spawn-display state. The second pass did not inspect it
    (`reports/20261020_0412_card_SSEL.md`, "CFOVM not closed by me"; planner `reports/20261020_0525_planner.md`).
- The body point's split: the eye holds 5.25 m behind / 4.13 m above the feet on Kamino (elevation 32.4 deg) where the
  set-up gives 35.0 deg; the distance matches to the millimetre, the angle is off by 2.6 deg - [H] the point the camera
  reads and the corpse settle after the set-up (the Hoth eye rose 4.20 -> 4.27 m over the 4 s).
- The other caller of `FUN_001bc5f8` is `FUN_001cdfc8`, the unit object's destructor (also a vtable word, 0x618e10):
  for a death camera still on that unit it re-initialises it with no unit and no killer from the player camera's
  matrix (`B2:77354-77378`) - the spawn-screen replay of §6 item 3 [D].

## 7. Other entries [V]

- **Pause menu "Respawn"**: a yes / no popup "Do you really wish to kill your current player?" (`ps2_flow_sheet.png` row 4
  right, EVMSG's `bf2_respawn_q.png`); Yes kills the soldier: the same death flow as §6 (the runs of §6 used it).
- **State 4, the class select while alive**: `FUN_00397f00` (`B2:392736-392739`) from the soldier update
  (`FUN_001f8548`, `B2:109297`, after a test against same-team objects in range), `FUN_00248c38` (`B2:164082`) and
  `FUN_0038cb28` (`B2:386520`). Back from 4 returns to play (state 5). [D] "while alive": two callers are unit-update code
  and the open / close block is skipped in state 4; which button and which object open it is not read here (§11).
- **Tickets 0**: the display does not open and an open one closes (`B2:392963`): no respawn.
- **Online only** (not built): Back from the spawn map to the side select, Spectate, team counts on the labels,
  "fewer humans" auto assign, the 15 s spawn delay (`SD+0x44 = 15.0` in `FUN_0039b748`).

## 8. BF1 (from earlier docs, not re-derived here)

BF1's screen is one C state machine (`FUN_002df2b0`, re-ambience-music §6.2: 0 team, 2 spawn point, 3 unit, 4 spawn
asked): CHOOSE YOUR SIDE with Left / Right between the two sides, the spawn map, the unit select (README "BF1 boot ->
in level"; `lead_tmp/re/pxc3/bf1_hot1_4side.png`, `_5spawnmap.png`). BF1 missions also call `AddCameraShot` (44 of 44
BF1 mission files [receipt:b3866474]). PLAN's shell is BF2's, so the port shows BF2's flow on both games' maps (§9, a reversible call).

## 9. The port against the PS2, and the build

| | PS2 (BF2, offline) | port today (`6dc5a68`) |
|---|---|---|
| level start | side -> post -> class -> spawn | the shell picks the armies and the player's team (`mission::pick`); the player is spawned alive at once |
| side choice in the sim | per player | none (team fixed before the battle) |
| post | spawn map pick, kept | SPSEL: `Sim::spawn_post_choice`, kept (done) |
| class | carousel, accept | `Sim::spawn_choice` moved by pad bits (done); no accept |
| respawn gate | 4.0 s, then the screens; spawn on X, no wave | 4.0 s timer AND the team's spawn wave (`SpawnWaves`), then automatic |
| death camera | §6.5: the body from 6.22 m (phase 1), then from the body toward the killer from 1.5 s (phase 2), 4 s | the chase camera |
| backdrop | camera shots at the start, the death view at a respawn, dimmed 0.51 | the battle, dimmed by the egui panel |
| `AddCameraShot` | 3..22 per BF2 conquest mission (first 16 kept) | not baked (`scripts/gen_missions.py` drops it) |

Build cards (filed by this pass; each pinned by a test on the sim's choice and a golden of the screen, the choices ride
the input log like `SPAWN_CLASS_*`):
- **SSELA (sim, value 5)**: a player slot that has not accepted is not on the field: at the level start (until side +
  post + class are accepted) and after a death (until the class is accepted, the 4.0 s timer first); a SPAWN_ACCEPT and a
  SPAWN_BACK pad bit and a per-slot screen state 0 / 2 / 3 in the sim (`Sim::spawn_stage`), the side choice
  (`Sim::side_choice`, Up / Down over team 1, team 2, Auto), the human skips the wave (the PS2's rule). Replays: the new
  bits ride the input log; the idle-player gates move (declare `sim`): a test harness that never presses accept needs an
  auto-accept switch for headless runs.
- **SSELB (ui + game, value 5)**: the three screens as the PS2 draws them over the backdrop (camera shots baked from
  `AddCameraShot`; the death view at a respawn; dim 0.51), the side preview (the highlighted team's first class) and the
  class line-up, the panel texts from the disc's locl keys, the help lines; goldens of each screen.
- **SSELC (game, value 4)**: the death camera of §6.5 for 4.0 s: phase 1 for every death, phase 2 toward the killer
  from 1.5 s after an enemy kill (the sim names the killer), the 6.0 m casts against the world.

**Auto Assign (a named deviation, reversible):** the PS2 offline keeps team 1. The maintainer asked for "a side (or random)", so
the port's Auto Assign draws the team with the PS2's own coin flip (`FUN_0036fc40`'s offline rule) from the sim's LCG.
Reverse: make Auto Assign keep team 1.

## 10. Checks run, and the verifier

- `re_check.py fn` on all 89 cited lines: each lies in the function named (`lead_tmp/ssel/checks/fn.txt`).
- `re_check.py absent b2 FUN_0039b828 0x0c0e6e0a --elf` [receipt:0400519d]: 3 jal words = 3 text callers
  (`FUN_003ecb98`, `FUN_003f1670`, `FUN_00407980`; the last two act only under `SD+0x41`, set only by the online auto
  branch). `re_check.py absent b2 FUN_0034d9a0 0x0c0d3668 --elf` [receipt:fcbf78d9]: 11 jal words = 11 text callers.
  `re_check.py absent b2 DAT_00825220` [receipt:15b1a896]: written only by `ScriptCB_MetagameSetSide`.
- `re_check.py writers`: `DAT_00600b4c` (`FUN_0019bfc8` cases, `FUN_004041a8`, `FUN_00428128`), `DAT_00600548` /
  `DAT_0060054f` (network setup only), `DAT_005ff825` (none; ELF word `0x5ff824` = 0x00000100, byte = 1).
- `re_check.py word`: the button tags (`0x633680..0x6336a8`), the screen names (`0x62f188`, `0x62f160`, `0x62f138`,
  `0x62f128`), the rule names (`0x5fa7c0` -> "instantaction", "mp", "campaign", "metagame"); `eedis.py` for the
  carousel's immediates and the auto-assign branch.
- Twin claims: none made (BF1 is cited from earlier docs, §8).
- **Verifier** (workflow `ssel-re-verify`, 7 agents at medium effort, claims and citations only;
  `lead_tmp/ssel/verify/VERDICTS.json`): 33 claims in 6 batches (state machine 8, side select 10, class + spawn 6,
  live rows 7, absence 2 to two independent verifiers): **35 verdicts, 26 SUPPORTED, 9 PARTLY, 0 NOT SUPPORTED.**
  Fixed from the PARTLY notes: the shot draw (two LCG steps, high halves OR'ed - the claim had one step), the open
  conditions (state 4 skips them, `FUN_004241a0`, the member is the viewport player's pool entry), the hero branch of
  the class copy, the side select's Up / Down handlers (copies 2-4 only), the 1e-4 timer class, the online
  `DAT_00600505` case, the display opening two updates after the timer reaches 0, Naboo's 4-update spawn, and the death
  camera's yaw (up to 6.7 deg in a third run). New from the notes: `FUN_00396a00` skips the side select where sides are
  fixed (§1); the demo flag `DAT_00600b15` (set only by `ScriptCB_LaunchDemo`) also reaches state 5 without a class.
  **The second verifier overturned one sub-claim**: X1's "the only other per-update team writes are 393263 and 393289"
  missed `B2:392291` (`FUN_00396a00`, the same value: the member's team); both verifiers support the core (offline Auto
  Assign calls no team setter), X2 both SUPPORTED.

- **Second pass (2026-10-20, the death camera, §6.5)**: `re_check.py fn` on 46 new cited lines (each in the function
  named, `lead_tmp/ssel/checks2/fn.txt`); `word` on the vtable slots 0x617d80/84/88, 0x617d28 and the constants
  0x5fc51c/520/524 (`checks2/word.txt`); `absent --elf` receipts for the call counts ([receipt:6b7e0f71]
  [receipt:09081883] [receipt:602956ad] [receipt:59c7f5bd]). Workflow `ssel-deathcam-verify` (4 agents at medium
  effort, claims and citations only; `lead_tmp/ssel/verify2_journal.jsonl`): 17 claims in 3 batches (object 7, starters 5,
  live 5) plus the 4 absence / count claims to a second reader: **21 verdicts, 17 SUPPORTED, 4 PARTLY, 0 NOT
  SUPPORTED; the second reader overturned none.** Narrowed from the PARTLY notes: the apply sets the zoom divisor,
  not the base FOV; `FUN_001d0a48` also calls online under further flags; the 6.22 m hold is to the feet at the death
  (the corpse settles 0.07-0.08 m); in 3 kills the eye drifted with the body before 1.5 s. Added from the notes: the
  destructor is also a vtable slot; `FUN_002379f0`'s skip on bit 4 of `E+0x228`; a vehicle-less, unit-less killer keeps
  the phase-1 points with the phase-2 weight.

## 11. Not settled [H]

- (Settled by the second pass, §6.5: it does turn to the killer, from 1.5 s. The first pass's three teleports into an
  empty spot drew no fire; putting the player beside the team-mate nearest an enemy did: 11 deaths in 4 runs.)
- Who writes the killer word `E+0x190`, and what the `+0xb8` test of `FUN_001bc5f8` excludes (a self-kill by one's own
  weapon is the likely case; not captured). What `FUN_00161c60` hits (read as a collision cast: the 6.22 m hold with
  nothing behind and the 0.2-1.2 m phase-2 eye with the floor under the lower cast fit it) [D].
- Why the explosion kill (`t11` death 1) kept no killer: the killer's unit may already have been gone, or the kill was
  credited to no member - not traced.
- Which button opens the class select at a friendly post while alive (state 4), and its range.
- (Settled by §4.1: the line-up's places and slide are read; the UI scene's perspective is fitted [M].)
- The campaign and Galactic Conquest open straight on the spawn map (§1, `FUN_00426e80`) - read, not captured.
- X2's one untraced path: `FUN_0034cca8` sets a member's player index without touching the ready byte; it is reached only
  through a vtable slot (`0x62d09c`) the verifier did not trace. The normal human setup clears the byte (`FUN_0034ffe0`).

## 12. Reproduction

- `lead_tmp/ssel/ssel_live.py --state S --plan "..." --out DIR` (docstring): boots a state, samples the display, the
  member, the camera and the player's soldier once per game update, runs keys timed in game seconds (the game clock stops
  in the pause menu: `wwait`). Runs: `t1_hot1` (states), `t2_hot1_death`, `t6_hot1_suicide` (rows + strip),
  `t7_bf2_kam1_cw`, `t7_bf2_nab2_cw`, `t8_kam1_suicide`; failed kill attempts `t3`..`t5`.
- `lead_tmp/ssel/sdstate.py [--png DIR] STATES...` reads the same fields from save states.
- `lead_tmp/ssel/disall.py <luac>`: a Lua 5.0 listing; `dis_*.txt` for the five in-game screens.
- Second pass: `ssel_live2.py` adds `front:HP` (the player beside the team-mate nearest an enemy, health HP) and
  `untildead:S` (repeat until killed; logs every soldier within 80 m with its bearing); `ssel_live3.py` / `ssel_live4.py`
  add the corpse's facing, posture, aim words and a second census 2.5 s after the death. Runs `t9_kam1_ai16_kill`
  (2 kills, strips, `fr/sheet_d0.png`, `sheet_d1.png`), `t11_kam1_ai16_kill3` (3), `t12_kam1_ai16_aim` (3); readers
  `cards/deathcam2.py` .. `deathcam4.py`; `t10_kam1_ai16_kill3_badrows` keeps only its events (its rows failed a short
  read).
