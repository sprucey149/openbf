# OPUS RE (2026-10-19) - [O] UNAUDITED

# RE: the map boundary - "Leaving Battlefield", the countdown and the kill (BF1 + BF2, card BNDY)

Card BNDY, lane 2 (started 2026-10-16, finished 2026-10-19). Corpora: B1 = `analysis/SWBFRONT.ELF-decomp.c`,
B2 = `analysis/SWBFRNT2.ELF-decomp.c`; ELF bytes read with `engine/artifacts/lead_tmp/bndy/tools/mdis.py` where Ghidra
drops a register argument. Tags: [V] read in the decomp / disassembly (lines cited), [D] derived from the disc data by a
named script, [M] measured on the PS2 (PCSX2 RAM), [H] not confirmed. Tools, inputs and outputs:
`engine/artifacts/lead_tmp/bndy/` (`tools/`, `points/`, `sheets/`, `checks/`, `wf/`). Extends `re-announcer.md` 2.4
(trigger T6) and `re-chunk-census.md` 5 (the `bnd_` chunk); `re-kill-floor.md` already noted the test is 2-D.

## 0. Answer table

| # | Question | Answer | Tag |
|---|---|---|---|
| 1 | Where the boundary comes from | The map LVL's `bnd_` chunk: `Boundary { Path(<name hash>) ... }`; each name is looked up among the world's paths (the first path of that name) and becomes one shape. Nothing else adds a shape | [O] C1-C4, A1 |
| 2 | The point test | Per shape, a CONVEX half-plane test over the path's edges (closed: last node -> first); a point is inside the boundary when it is inside ANY shape; x and z only; no shapes = never outside | [O] C5-C7 |
| 3 | Which maps have one | 22 of the 32 baked maps; NOT BF1 YAV1 / YAV2 (no `bnd_`), NOT BF2 COR1 DAG1 DEA1 MUS1 MYG1 NAB2 TAN1 UTA1 (empty Boundary scope). Showcase: BF1 NAB1 (1 shape), BF1 TAT1 (1), BF2 HOT1 (3); none on BF1 YAV2, BF2 UTA1, BF2 MYG1 | [D]+[M] |
| 4 | The rule | Every unit (AI included) is tested each update - its soldier and what it drives. Outside -> a timer starts at BF1 10.0 s / BF2 6.5 s; at the start the local human hears his team's leaving line and sees "LEAVING BATTLEFIELD" (pulsing); the timer runs down by the update's dt; below 0 the unit (and its vehicle) is killed; back inside -> the timer is cleared and the next excursion starts again from the full time | [O] T1, T2 + [M] |
| 5 | The BF2 2.17 s | A flyer class with property 0xc6e9c80b = 1: on the disc only three guided-rocket ordnance classes (a steered rocket the player flies) - not starfighters | [O] T3 + [D] |
| 6 | The "forced" flag (B2 +0x660 bit 7) | The BF2 FLYER's fly-height flag: set each update when a flyer's height is below `MinPlayerFlyHeight` or above `MaxPlayerFlyHeight` (Lua, per mission; defaults -200 / +200). Such a flyer counts as OUTSIDE even inside the 2-D boundary. Belongs to card VFLY (the flyer) | [O] T3b |
| 7 | Do bots turn back at the boundary | No foot-soldier code reads the test; one BF2 AI flyer function uses it to refuse a placement point outside (4.) | [O] A2 |

## 1. The data - where the shapes come from

- C1 [V][O] The level loaders hand the chunk `bnd_` (0x5f646e62) to the boundary reader: B1 FUN_00175d00 ->
  `… B2 FUN_00177058 -> `FUN_0034c3b0(DAT_005feca8, chunk)`
  (B2:18380-18381).
- C2 [V][O] FUN_002954a8 (B1:220441-220495) / FUN_0034c3b0 (B2:343324-343372): for a statement hashed 0x54eacca7
  (FNV("boundary")) each child statement hashed -0x7b78b2ca (= 0x84874d36, FNV("path")) calls the push
  (B1 FUN_00295678 / B2 FUN_0034c4f0) with its first argument, the path NAME's FNV (B1:220487-220489, B2:343363-343366).
- C3 [V][O] The push (B1:220500-220518, B2:343373-343391) passes that hash on in a1 untouched (ELF 0x295678-0x2956d4 /
  0x34c4f0-0x34c54c: a0 = 0x676110 / 0x744ac0, a1 not written before the `jal`) to the path lookup (B1 FUN_00472290,
  B2 FUN_0055d008); a found path -> a 12-byte shape (FUN_002957b8 / FUN_0034c638) linked at the list head (*object);
  a name not found adds nothing.
- C4 [V][O] The lookup returns the FIRST path of that name walking the world's path list forward (path+0x10 = the name
  hash); BF2 registers paths at the list's tail (FUN_0055da00 -> FUN_0055cfd8, B2:693883-693891), so the first
  registered path of a name wins. Data: BF2 KAS2 and YAV1 carry two `boundary1` paths; the RAM shape is the first in the
  manifest's order (R1). BF1's insert is not read [H] - no BF1 map has a duplicate boundary name, so it changes nothing.
- A1 [V][O] The push has one caller in each game (FUN_002954a8 B1:220483, FUN_0034c3b0 B2:343366), one `jal` word in
  each ELF (B1 0x0c0a559e, B2 0x0c0d313c), no `j` word and no data word with its address; the shape builders are
  called only from the push [receipt:e88fecd8]. So the `bnd_` chunk is the ONLY source of shapes and a map without one,
  or with an empty scope, has no boundary for the whole battle (verifiers: both SUPPORTED, each ran its own ELF scan).
- C8 [V][O] The object: B1 FUN_00295320 allocates 0x14 bytes, FUN_00295390 zeroes the list head and builds two 8-byte
  voice slots at +4 / +0xc (team 1 / team 2); FUN_00295358 -> FUN_00295400 frees the shapes and the slots and clears
  the global (B1:220356-220440). The global is written only there (`re_check writers`: B1 FUN_00295320 / FUN_00295358,
  B2 FUN_0034c230 / FUN_0034c268). Every user of the global: B1 FUN_00175d00, SetOutOfBoundsVoiceOver, FUN_00295320,
  FUN_00295358, FUN_00297e60, FUN_002afa90; B2 FUN_00177058, SetOutOfBoundsVoiceOver, FUN_002faf90, FUN_0034c230,
  FUN_0034c268, FUN_00350660, FUN_00371ce0, FUN_00391fe0 [receipt:d04a43b0].
- D1 [D] `tools/bndmaps.py` (re-run 2026-10-19, `checks/bndmaps_rerun.txt`; decoder `lead_tmp/fxc/tools/bnd.py`):

| Game | Map | Shapes (path: nodes) | Convex | Polygon / admitted area m2 |
|---|---|---|---|---|
| BF1 | BES1 | Boundary: 8 | NO | 811,032 / 712,435 |
| BF1 | BES2 | boundary1: 10 | yes | 63,675 |
| BF1 | END1 | boundary1: 6 | yes | 258,370 |
| BF1 | GEO1 | boundary1: 6 | yes | 371,529 |
| BF1 | HOT1 | boundary1: 8 | NO | 376,186 / 375,324 |
| BF1 | KAM1 | boundary1: 9 | NO | 247,898 / 246,666 |
| BF1 | KAS1 | boundary1: 13 | yes | 380,448 |
| BF1 | KAS2 | Boundary1: 10 | yes | 215,471 |
| BF1 | NAB1 | boundary1: 8 | yes | 208,841 |
| BF1 | NAB2 | Boundary: 7 | yes | 115,060 |
| BF1 | RHN1 / RHN2 | Boundary: 7 / 8 | yes | 137,854 / 45,894 |
| BF1 | TAT1 | boundary: 6 | yes | 360,818 |
| BF1 | YAV1, YAV2 | none: no `bnd_` chunk in YAV/YAV1.LVL, YAV2.LVL, YAV2_H.LVL (NAB1.LVL control: 1 hit) [receipt:93874330] | - | - |
| BF2 | END1 | boundary1: 6 | yes | 258,370 |
| BF2 | FEL1 | warning1..4: 12 each | yes | 14,998 + 10,416 + 16,236 + 30,000 (overlapping) |
| BF2 | GEO1 | boundary1: 12 | yes | 246,432 |
| BF2 | HOT1 | boundary1..3: 12 each | yes | 383,187 + 68,403 + 57,876 (overlapping) |
| BF2 | KAM1 | one path (name hash 1134563362): 12 | yes | 426,387 |
| BF2 | KAS2 | boundary1, boundary2: 12 each | yes | 79,800 + 115,968 |
| BF2 | POL1 | pol1: 12 | yes | 93,793 |
| BF2 | TAT2 | boundary1: 6 | yes | 58,094 |
| BF2 | YAV1 | boundary1: 9 | yes | 141,543 |
| BF2 | COR1 DAG1 DEA1 MUS1 MYG1 NAB2 TAN1 UTA1 | none: a `bnd_` chunk with an empty Boundary scope | - | - |

  Not in this table, read off the LVLs for issue #59 (2026-11-01): BF1 TAT2 names `boundary1`, BF2 TAT3 has no `bnd_`
  chunk. Every name above came out the same. `game::map_boundary` holds the result. Its test agrees with all 404 rows of
  `points/*.result.json` that read inside or outside (`remote_droids::the_boundary_reads_as_the_ps2_read_it` keeps 14).

  Every shape is wound clockwise in (x right, z up), the side the test admits. D2 [D]: every conquest post and every
  node of every post's SpawnPath lies inside on the 30 maps in the table (one BF2 KAS2 object,
  `baserush`, outside: another mode's).
- R1 [M] RAM against the transcription, 32 baked maps, one in-battle save state each (`tools/cmpshapes.py`, re-run
  2026-10-19 = the 10-16 output byte for byte, `checks/cmpshapes_rerun.txt`): 0 disagree; 271 edges, largest float
  difference 6.2e-5; BF1 YAV1 / YAV2 and the eight empty BF2 maps hold an empty list in RAM. The port's engine space is
  the PS2's RAM frame (BF2 z negated by mungefs as FUN_0055d060 does).

## 2. The test

- C5 [V][O] The shape build (B1 FUN_002957b8 B1:220569-220605, B2 FUN_0034c638 B2:343442-343480): n edges for n nodes;
  edge 0 = (node[n-1], node[0]), edge i = (node[i-1], node[i]); the edge record (B1 FUN_00295960, B2 FUN_0034c7f0) is
  {a.x, a.z, a.z - b.z, b.x - a.x}.
- C6 [V][O] The shape test (B1 FUN_002958f8 B1:220633-220655, B2 FUN_0034c780 B2:343506-343530): the point is in the
  shape iff `(x - a.x)(a.z - b.z) + (z - a.z)(b.x - a.x) <= 0` for EVERY edge - an intersection of half-planes. On a
  non-convex path (BF1 BES1, HOT1, KAM1) the PS2 admits less than the drawn polygon (D1's admitted area).
- C7 [V][O] The outside test (B1 FUN_002956d8 B1:220520-220545, B2 FUN_0034c550 B2:343393-343418): no shapes -> 0
  (never outside); in any shape -> 0; in none -> 1. Reads only x and z: no height term in the 2-D test (the BF2 flyer's
  height band is a separate flag, 4).
- M1 [M] The PS2's own verdict at test points (`tools/bnd_live.py`: the player held at each point 20 m above his start
  for 0.5 game s, health pinned; the member's timer read twice 0.2 s apart; back home between points; points from
  `tools/bnd_points.py`: per edge 3 m out / 3 m in at the midpoint, per vertex 4 m out on the bisector, on a non-convex
  path 6 points inside the polygon but outside the half-plane region, on a map with no boundary 24 points on rings of
  300 / 600 / 900 m round the posts' centre):

| Map | State | Points | PS2 outside | Agree | Notes |
|---|---|---|---|---|---|
| BF1 NAB1 | bf1_nab1_walk_cp0_cp1_b000 | 24 | 16 | 24 | |
| BF1 TAT1 | tat1_walk_cp0_cp10_b000 | 30 | 18 | 30 | |
| BF1 KAM1 | bf1_kam1_cp0_cp2_beta1_spawn | 33 | 25 | 33 | all 6 non-convex points OUTSIDE on the PS2 |
| BF1 HOT1 | bf1_hot1_cp2_cp6_postA_spawn | 30 | 23 | 30 | all 6 non-convex points OUTSIDE on the PS2 |
| BF1 YAV2 | bf1_yav2_cp0_cp2_postA_spawn | 24 | 0 | 24 | no boundary: never outside, up to 900 m out |
| BF2 HOT1 | bf2_hot1_walk_cp0_cp5_b000 | 108 | 52 | 108 | union of 3 shapes: edge points inside another shape are inside |
| BF2 UTA1 | bf2_uta1_walk_cp0_con_cp7_b000 | 23 | 0 | 23 | no boundary |
| BF2 MYG1 | bf2_myg1_walk_cp0_cp4_con_b000 | 24 | 0 | 24 | no boundary |

  296 of 296 agree (and 108 of 108 again on BF2 HOT1's full-battle state, `live_bf2ai.log`); every "outside" point read
  back -1 within 0.033 s (one update) of returning home. Results `points/*.result.json`, logs `live_chain1.log`,
  `live_chain2.log`. Sheets: `sheets/<map>.png` (all 32 maps: the baked terrain from above, the drawn path in white,
  the admitted region green, posts cyan, each test point green = PS2 inside / red = PS2 outside).

## 3. The timer and who is tested

- T1 [V][O] BF1 FUN_00297e60(dt, member) (B1:222622-222705), from the member update FUN_00295c40 (B1:220963) whenever
  member+0xb4 (its soldier) is set: the member's three entities +0xb4 / +0xb8 / +0xbc [D: the soldier, the vehicle he
  drives, a third controllable] are tested at their matrix positions; an inside one is dropped. None left ->
  timer member+0xe4 = -1. Else: timer < 0 -> 10.0 (0x41200000) and, for a local human (FUN_002ff158(member+0x9c) >= 0),
  his team's line `FUN_00295750(obj, member+0xa0)`; then timer -= dt; below 0 -> every entity still outside is killed
  (`FUN_001bbc98(health+8, {0}, 1)`). The third argument 1 RUNS the invincibility test: FUN_001bbc98 returns
  without damage when DAT_00519060 (`/invincible`) is set and the target is a human (controller index >= 0) - so the
  cheat SPARES a player from the boundary kill, unlike a death region (argument 0, re-kill-floor 1f) [V: FUN_001bbc98's
  *(decompiler detail omitted)* branch; the 10-16 draft said the opposite]. BF2 passes 1 in the same position [D: the same
  argument; BF2's test not opened].
- T2 [V][O] BF2 FUN_00350660(dt, record) (B2:346642-346743), from FUN_0034cea8 (B2:343971) when record+0x148 is set: the
  same over +0x148 / +0x14c / +0x150; timer record+0x184 = 6.5 (0x40d00000), or 2.1667 (0x400aaaab) when one of the
  entities is a flyer (DAT_00689368, flyer = entity - 0x270) whose class (+0x700) has bit 1 of +0x1208 (4); below 0 ->
  each entity still counted is killed through its health object (FUN_001df050(1.0, ...), 1). No sound here (T5).
- T2b [V] The third slot (issue #59; closes section 7's first row for it): the record's +0x150 / the member's +0xbc is
  written by one setter (B1 `FUN_00296e80`, B2 `FUN_0034ef98`), set when a pilot of type 3 (a remote) takes over and
  cleared when control goes back (B2 `FUN_0024c560` B2:166192, `FUN_0024c778` B2:166225, `FUN_003dc908` for pilot
  type 3 B2:436502-436504; B1:120766, 120781). So a droid is tested only while someone flies it. Left flying, or
  handed back outside, it is not, and its owner's timer clears.
- M2 [M] Death trials (`bnd_live.py --death`: held at the first outside point, health NOT pinned, one row per game
  update): BF1 NAB1, TAT1, KAM1: the first outside update reads 9.9666 (= 10.0 - one update's dt, 0.0333 s), the player
  dies at the update the timer goes below 0 (-0.0099), 299 updates later = 9.98 s after the first outside row (10.0 s
  after the excursion began). BF2 HOT1: first row 6.4667, death at -0.0333, 195 updates later = 6.51 s. So: the timer
  is set AND decremented in the first outside update; the kill falls in the update it goes negative.
- M3 [M] An AI unit is tested like the player (`bnd_live.py --ai`: one living AI soldier held outside with health
  pinned): its member's timer left -1 and ran down (9.20-9.23 after 0.8 s) on BF1 NAB1, TAT1, KAM1 and HOT1, and read -1
  again 1 s after it was put back; BF2 HOT1 on a full-battle state (`bf2_hot1_cp1_cp6_postA_spawn`, the walk states
  have no AI): an AI record's timer went -1 -> 5.73 (= 6.5 - 0.77) in 0.8 s (`live_bf2ai.log`; the same run's 108
  points agreed again). So bots outside die like the player.

## 4. The two BF2 flyer cases

- T3 [V][O] The 2.17 s case: bit 1 of class+0x1208 is set by the class property 0xc6e9c80b (FUN_0020fd58,
  B2:125688-125697; also class+0x59c = 3). [D] `tools/keyclasses.py bf2 0xc6e9c80b`: only three disc classes author it
  = 1, all ordnance - com_weap_award_rocket_launcher_ord, com_weap_veh_fly_guided_rocket_ord,
  com_weap_veh_guided_rocket_ord: a guided rocket the player steers gets 2.17 s; every starfighter 6.5 s. The key's name
  is not cracked.
- T3b [V][O] (CORRECTED by both verifiers; the 10-16 draft called it dead) The flyer's bit 7 of +0x660, which keeps an
  entity counted outside while it is inside (B2:346692) and keeps the HUD up (B2:389591), is the FLY-HEIGHT flag:
  the flyer update FUN_00202d08 clears it every update (B2:116581) and sets it (B2:118385-118395) when the flyer's
  height (+0x154) is below DAT_00697e50 or above DAT_00697e54, under `(+0x330 >= 0) || (+0x324 == 0)`
  [H: +0x330 >= 0 = a human pilot; +0x324 == 0 = no pilot]. DAT_00697e50 / 54 are written only by the Lua
  SetMinPlayerFlyHeight / SetMaxPlayerFlyHeight (B2:32240, 32250) and FUN_002a4030's defaults -200.0 / +200.0
  (B2:225630-225631) [receipt:40a824aa]. In the same update the flyer's height is clamped to [min - 0.2, max + 0.2]
  (B2:119056-119066: the PLAYER limits when +0x330 >= 0, else DAT_00697e48 / 4c, the AI's SetMin/MaxFlyHeight).
  [D] the BF2 conquest scripts set (`scripts/gen_missions.py`): MaxPlayerFlyHeight COR1 25, DAG1 20, DEA1 72, END1 43,
  FEL1 53, GEO1 50, HOT1 70, KAM1 102, MUS1 90 / 84.16, MYG1 20, NAB2 25, POL1 55, TAT3 90, UTA1 29.5, YAV1 14; KAS2 and
  TAT2 set only the AI's MaxFlyHeight (70, 40), so the player keeps +200. So a human flyer at the ceiling sits above
  it (clamped at +0.2) and gets the leaving countdown - on UTA1 and MYG1 too, which have no 2-D boundary. [H] whether
  a player can hold a flyer at the ceiling for 6.5 s on the PS2 (no live trial).
- A2 [V][O] (narrowed by both verifiers) The outside test's callers, all of them: B1 FUN_00297e60 (222656),
  FUN_002afa90 (240070); B2 FUN_00350660 (346687), FUN_00371ce0 (369690), FUN_002faf90 (288495) - jal counts 2 / 3 in
  the ELFs, the shape test called only from the outside test, no `j` or data words [receipt:b9ed43f3]
  [receipt:2c3debca]. B2 FUN_002faf90, called from FUN_00202880 (B2:116373, in the flyer code beside FUN_00202d08),
  builds a point 300 m (0x43960000) along a matrix row from a flyer's position (at least 30 s into the battle) and
  returns 0 when that point is outside - an AI flyer placement test [H: which manoeuvre]. No foot-soldier steering or
  route code calls either test: bots do not turn back at the boundary (the port's BNDS barrier walk is ours).
- T6 [V] The two other readers test an object's position through a handle: B1 FUN_002afa90 (240056-240070), B2
  FUN_00371ce0 (369675-369690); their single callers (B1 FUN_002b0780, B2 FUN_00373138) are gated on bit 3 of a flag
  word. [H] the empty-vehicle decay (`re-vehicles` VPAD notes): an empty vehicle outside skips its idle reset. Not
  needed for the soldier rule.

## 5. What the player sees and hears

- T4 [V][O] BF1 HUD element: text Locl 0x3a1fed50 `game.tooltips.leaving_battlefield` ("LEAVING BATTLEFIELD"; BF2
  "Leaving Battlefield", `lead_tmp/re/evmsg/keys.py`), font 0x540aa0 (gamefont_large), at (0.5, 0.5) of the screen
  (FUN_002d88a8 / FUN_002d8968, B1:264252-264284; the fallback string at 0x540ab0 is
  'FIXME: Localize game.tooltips.leaving_battlefield', `re_check.py word b1 0x540ab0 --str`); FUN_002d89c0
  (B1:264297-264330): shown while the HUD member's (elem+0x90 -> +0x27d4) timer >= 0; on the first shown frame alpha
  254, rate 0; each frame alpha += dt * rate, rate += dt * (128 - alpha) * 500 (an undamped spring round 128: a 0.28 s
  period, alpha 2..254), clamped [2, 255] into the colour's alpha byte; hidden when the timer is < 0.
- T5 [V][O] BF2 FUN_00391ed8 / FUN_00391fa0 / FUN_00391fe0 (B2:389520-389625): the same element and spring; the LINE is
  played here, on the frame the element first shows: FUN_0034c5c8(obj, the entity's team nibble) - once per excursion,
  for the local human only (the HUD reads the local player's record, *(elem+0x90)+0x1314).
- T1b [V][O] The line: the object's slot (team - 1) is played (B1 FUN_00295750 -> FUN_00354fe0, B2 FUN_0034c5c8 ->
  FUN_0043ceb0); the Lua `SetOutOfBoundsVoiceOver(team, name)` fills it (B1:32845-32858 -> FUN_00295788; B2 the same on
  DAT_005feca8). The names per mission: `lead_tmp/annc/bf1_calls.txt` / `bf2_calls.txt`; five BF1 scripts set none
  (no line, the HUD text still shows). Sound locks (audit ANNC): BF2 `allleaving` PlayInterval 10, every `repleaving`
  take `(id, 1, 1, 0)`; a request inside the interval plays nothing.

## 6. Checks run (RE discipline)

- `re_check.py fn` on every cited line (`checks/fn_b1.txt`, `fn_b2.txt`): all in the functions named (one range fixed:
  FUN_002d89c0 starts at B1:264297, not 264290).
- `re_check.py absent`: [receipt:b9ed43f3] (outside-test callers + jal words), [receipt:2c3debca] (shape test + data
  words), [receipt:e88fecd8] (push callers + jal words + data words), [receipt:d04a43b0] (users of the object
  globals), [receipt:40a824aa] (the fly-height globals), [receipt:93874330] (no `bnd_` in the Yavin LVLs, NAB1 control).
- `re_check.py writers` DAT_00518314, DAT_005feca8, DAT_00697e50, DAT_00697e54 (above). `callers` FUN_00297e60 (1),
  FUN_00350660 (1). Constants read in the decomp as hex words (0x41200000 = 10.0, 0x40d00000 = 6.5, 0x400aaaab =
  2.16667, 0x43960000 = 300, 0xc3480000 / 0x43480000 = -200 / 200, 0x437e0000 = 254); `re_check.py word b1 0x540aa0
  --str` = 'gamefont_large'.
- The twin: every C-claim cites both games' functions; both verifiers opened both.
- Verifier: ONE workflow (`wf/claims.json`, `wf/verify.json`), 19 claims in 3 batches, every claim to TWO independent
  verifiers (6 agents, medium effort, claims and citations only). Both verifiers agreed on every claim: 16 SUPPORTED
  by both; A2 and T6 PARTLY by both (A2's "no AI reads it": B2 FUN_002faf90; T6's "vehicle" label not shown by the
  cited code) - both narrowed above; T3b NOT SUPPORTED by both (the +0x660 bit 7 writer B2:118395) - rewritten above
  from the cited lines. The second verifier overturned no claim the first supported. The cheat reading in T1 (the
  third argument) was checked after the workflow, in FUN_001bbc98 itself.

## 7. Stayed [H]

- That +0xb8 / +0x14c is the driven vehicle: read as "the soldier and what he drives"; the BF1 code kills a driven
  vehicle with its driver. +0xbc / +0x150 is the remote (T2b).
- The fly-height pilot gate (+0x330 / +0x324) and whether the ceiling countdown is reachable in play (card VFLY).
- FUN_002faf90's manoeuvre; the vehicle decay readers (T6).
- BF1's path-list insert order (no BF1 map has a duplicate boundary name).

## 8. For the build (card BNDY phase build)

- Bake: per map the `bnd_` path NAMES in order (first path of a name), into the manifest beside `death_regions`; the
  paths are already baked (`mungefs::path`, engine space = RAM frame).
- Rule (core): shapes = per path its n edges as C5; inside = any shape's every-edge test (C6, `<= 0`), x and z only;
  per unit a timer (None = inside): first outside tick -> T (6.5 s BF2 rule on every map per PLAN; BF1's 10.0 recorded
  as data) and in the same tick -= dt; < 0 -> killed with its driven vehicle; inside -> cleared. Every unit, AI
  included (M3). The cause is one the invincibility cheat spares (T1).
- Player: "Leaving Battlefield" (Locl 0x3a1fed50) centred, alpha spring of T4; the team's leaving line from the
  mission's `SetOutOfBoundsVoiceOver` (ANNC's `out_of_bounds` field) once per excursion, for the local human, with
  the `allleaving` / `repleaving` locks.
- The guided rocket's 2.17 s (T3) waits for the guided rocket's flyer (card VFLY).
- T3b's height test sits in the flying state's case of the update's state switch (B2:118249 `case 2`, up to B2:118673), so
  a craft on the pad never sets the flag. That matters: `uta_fly_ride_gunship` pads stand at y 83 on BF2 UTA1 (ceiling 29.5),
  46 to 61 on MYG1 (20) and up to 125 on KAM1 (102). [H] what a takeoff from those pads does on the PS2: the height clamp
  (B2:119056-119066) would pull the craft down to the ceiling. No live trial.
- Built for flyers (2026-11-01, `Sim::tick_flyers`): T1, T2 and T3b (flying state only) for a craft with a crew, the HUD line and the
  commander's line through `Sim::is_leaving`. PROVISIONAL: one timer on the craft rather than one per crew member. Soldiers
  on foot and ground vehicles still do not count, and a human who jumps out leaves the countdown behind, which the PS2's
  soldier slot would not. The guided rocket's 2.17 s is not built here. Headless tests only.
