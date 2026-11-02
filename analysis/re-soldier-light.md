# OPUS RE (2026-10-16, lane 2) - [O] UNAUDITED: why the port's BF1 soldiers are darker than the PS2's (card SBLT)

**The question** (card SBLT; `re-spec-gloss.md` §8): with the specular pass removed, the port's droid bodies read darker
than the PS2's at the same pose: BF1 KAS2 SBD luma mean 26 vs 54, BF2 KAM1 battle droid 39 vs 48 (2026-09-29). Which
term (ambient, sun, the light sum, the texture), and is it a pose artefact?

Tags: [V] read (lines / ELF / disc bytes cited) or measured (files cited); [V-RAM] read in PCSX2 save states; [V-PS2] a
PCSX2 frame of a patched state; [D] derived; [H] hypothesis. Scratch, scripts, frames: `engine/artifacts/lead_tmp/sblt/`
(below `sblt/`). B1 = `analysis/SWBFRONT.ELF-decomp.c`, B2 = `analysis/SWBFRNT2.ELF-decomp.c`.

## 0. The answer

| # | term | the PS2 | the port (main c3bff2a) | size on BF1 KAS2's SBD (no pass) | maps |
|---|---|---|---|---|---|
| 1 | **the soldier's ambient on a BF1 map without `CharacterAmbientColor`** | (128,128,128): the mission reset's value, which `AmbientColor` can no longer reach (§1) | `AmbientColor` (78,87,91 on KAS2) | PS2 51.2 -> 38.8 with the port's value; port 28.1 -> 43.8 with the PS2's (idle pose) | BF1 KAS2, **NAB1**, **YAV2** (+ their `_H`) |
| 2 | the sun on the soldier | C·((1+n·l)/2)⁴, bytes/128, per vertex, the normal turned by its bone (§3) | the same form and scale | back-lit: equal (29.1 vs 29.7); overhead / front-lit: PS2 2.6-7 x (§3.2); real sun 12.7 vs 3.7 | open [H] |
| 3 | the soldier's LUMI tint (a darkening) | colour = min(1.2·LUMI + 0.3, 1) at his feet, on maps with `DomeInfo` `TerrainColorDarkening` (§4) | none (colour 1.0) | none on KAS2 (flag off) | BF1 END1, **NAB1**, **TAT1**, TAT2; BF2 END1, KAS2 |

Not a term: the omnis (none in reach at the pose: 51.2 -> 51.4), the light sum's colour space (LSUM landed; the
ambient-only term matches, 35.3 vs 33.0), the per-pixel vs per-vertex evaluation (§3.3), the heading (§2).

**BF2 KAM1 is no longer darker.** After LSUM and B1DK the port's battle droid in its own idle clip reads 62.2 against
the PS2's 55.2 / 57.4 (two frames; the PS2's droid wears the orange engineer skin the port does not have, card USKN)
(`sblt/measure_tables.txt`). BF2 has no character-ambient split (§1.6).

**What to build** (card SBLT -> build): BF1, no `CharacterAmbientColor` -> the character ambient is (128,128,128)
(bf1_kas2 / bf1_nab1 / bf1_yav2). Term 3 is card SLUMI. Term 2 stays open with its instruments, card SSUN (§3.4).

## 1. BF1's soldier ambient slot [V][V-RAM][O]

1. **The three ambient words.** `FUN_00493298` (the `AmbientColor` setter) writes `_DAT_0069c570` always and
   `_DAT_0069c578` / `_DAT_0069c580` only while `DAT_00522a88` / `DAT_00522a8c` are 0 (B1 555604-555611).
   `FUN_00493328` (`CharacterAmbientColor`) sets `…
   `FUN_00493378` (`VehicleAmbientColor`) the same for 0x8c / 0x580. The light-set builder `FUN_00493cf0` takes 0x578
   for non-static draws without a shadow volume - soldiers (556042-556043; `re-model-lighting.md` §12, whose GEO1
   knock-out turned the soldier black).
2. **The reset.** `FUN_00493670` clears both flags (555812-555813) and calls `FUN_00493298` with 0x80,0x80,0x80,0xff
   (555814-555821): all three slots (128,128,128). Its one caller is `FUN_004790e0` (540537), whose one caller is
   `FUN_00173a98` (16187), which loads `MISSION.lvl` (0x527350) and is called with 0 (the shell, `FUN_00174838` 16464)
   and 1 (a mission, `FUN_00174bd8` 16627). No data word holds 0x493670 or 0x4790e0 [receipt:bb3a1463].
3. **The flag gets set by a HUD draw.** `FUN_004b8e78` saves the 0x570 / 0x578 / 0x588 bytes (578283-578299), sets its
   own rig through `FUN_00493298(0x6fd1c8)` and `FUN_00493328(0x6fd1c8)` (578306-578309) and restores through
   `FUN_00493298` / `FUN_004930d8` / `FUN_00493128` / `FUN_00493328(saved 0x578)` (578462-578465). So it leaves 0x578
   as it found it and `DAT_00522a88 = 1`. The scene render `FUN_00479458` calls it every time, unconditionally
   (540750; also per view at 540744 when `DAT_00677648[i]` and `DAT_00677690[i]` are set; a third caller is
   `FUN_002ef438`). The only writers of `DAT_00522a88` are `FUN_00493328` (= 1) and the reset (= 0)
   [receipt:4e7a6368]; of 0x578 `FUN_00493298`, `FUN_00493328` and the static initialiser `FUN_00494d18` (80 80 80 ff)
   [receipt:f90cab11]; the lightning effect `FUN_00378120` and `FUN_004b8e78` write it only through `FUN_00493328`.
4. **So** [D]: once a frame has been drawn after the mission's reset, `AmbientColor` no longer reaches 0x578. A map that
   authors `CharacterAmbientColor` writes it there; one that does not keeps the reset's (128,128,128). Where in the load
   the first frame falls relative to the sky's parse is not read [H] - the RAM below settles the outcome.
5. **RAM, every BF1 reference state** (`sblt/tools/ramcensus.py` -> `sblt/ramcensus.txt`) [V-RAM]: 0x578 =
   (128,128,128) on every map whose sky has no `CharacterAmbientColor` - KAS2, NAB1, NAB1 historical, YAV2 - and the
   authored colour on the other 11 maps; both flags 1 in every battle state. Exceptions, both read: one KAS2 row reads
   192 everywhere (saved inside `FUN_004b8e78`'s rig, the B1DK trap) and three of KAM1's four rows read 0x578 =
   (149,154,176) with 0x570 (109,115,121) - saved during a lightning flash, whose `FUN_00378120` swaps the three words
   (memory: lightning moves PS2 frames); the fourth reads the authored (145,143,161). The disc
   (`sblt/tools/disckeys.py` -> `sblt/disckeys.txt`): KAS2 authors `AmbientColor` (78,87,91) and `VehicleAmbientColor`
   (58,67,70), no `CharacterAmbientColor`; the same three maps (+ `_H`) are the ones without it; every authored value
   equals RAM.
6. **BF2** has no such key: 0xcda12257 is absent from B2 in both forms [receipt:b857caf5]; it builds one ambient for
   every draw kind (`re-model-lighting.md` §3; `FUN_00597e98` copies the `lght` global ambient into every set).
7. **The port** bakes `character_ambient: None` for bf1_kas2 / bf1_nab1 / bf1_yav2 and then lights the soldier with the
   map's `AmbientColor` hemisphere (`shader.wgsl` `shade_unfogged`, `LIGHT_CHARACTER`). Also the hue: KAS2's
   `AmbientColor` is blue-grey (B/R 1.17) where 128 is neutral - WADE's stormtrooper helmet, port (85,94,97) against
   the PS2's (121,122,122) (`pcsx2/ref/bf1_kas2/wade/pairs/s094.png`, FINDINGS F67), is the same term [D].

## 2. The pose [V-RAM]

The PS2's SBD at `bf1_kas2_cw_cis_cp4_cp11_dock_two_spawn` stands at (196.305, -1.020, 107.750) facing exactly along
the camera (heading -127.09 = the camera's; `sblt/tools/b1pose.py`); BF2 KAM1's engineer at (8.051, 78.411, -79.584),
heading -26.46 = the camera's (`sblt/tools/b2pose.py`). DSHN's port plant (facing along the camera) was right. Its
gallery pose was not: the SBD's own `human_sbdroid_stand_idle_emote` matches the PS2's arms; it moves the port's
mean by < 1.5 levels (29.2 -> 28.1).

## 3. The sun on the soldier [V][V-PS2]; the gap [H]

1. **The VU path** [V]: BF1's rigid one-bone path (flags & 0x4000, VU1 0x5ff-0x602) turns each vertex's position by its
   bone's four rows and its NORMAL by rows 144-146 (0x5ac-0x5c3, `dshn/bf1_vu1.txt`) before the lit loop; the lit loop
   squares d twice (0x1cb, 0x1cc) - the static models' d⁴ (`re-sun-direction.md` §1.8).
2. **The PS2 sun swept in RAM** (ambient 0, `sblt/ps2/mk1.py` `sunT` / `sunS`; whole droid, 3-px-eroded mask; the
   port in the SBD's idle clip, `sblt/tools/port_kas2.sh`) [V-PS2]:

   | sun | T0 behind | T45 | T60 | T90 overhead | T120 | T135 | T180 front | S±45 side | real |
   |---|---|---|---|---|---|---|---|---|---|
   | PS2 | 29.1 | 28.0 | 24.8 | 17.3 | 10.5 | 7.4 | 3.0 | 15.8 / 13.4 | 12.7 |
   | port | 29.7 | 23.8 | 17.5 | 6.7 | 2.1 | 1.0 | 0.1 | 6.7 / 7.9 | 3.7 |

   No light: PS2 2.2 (a halo of the bright floor at the silhouette, the frame filter), port 0. The back plate alone
   follows d⁴ with the plate tilted ~5 degrees up (0.598 / 0.392 / 0.098 / 0.019 at 45 / 60 / 90 / 120 degrees, d⁴
   0.61 / 0.39 / 0.09 / 0.007; d² and Lambert do not fit), and its T0 level is the port's (31.4 vs 35.9). No left / right
   mirror (both light the same half for each side).
3. **Not per-vertex vs per-pixel**: the sun computed per vertex in a scratch `vs_skinned` moves the port's droid by
   0.2-0.5 levels (`sblt/port/kas2_vtx_*`, `sblt/tools/vtx_sun_shader.diff`).
4. **Open [H]**: the PS2 droid takes more sun from overhead and in front. Candidates: the legs (the PS2's thighs catch
   overhead light, the port's idle clip shows thin struts - `sblt/sheet_T90_T135_x3.png`), the pose of the parts the
   gallery clip does not match, a normal treatment in the skinned path not read here. Next instrument: pose the port
   with the PS2's own bone matrices (rows 144+ in the soldier's VU upload, or the animator's palette in RAM).

## 4. BF1 / BF2: the soldier's LUMI tint [V][O]

- B1 `FUN_002013a0` (the soldier draw): tint = 1, or the terrain LUMI at the soldier's (x, z) (`FUN_00473bc0`, bilinear
  LUMI/255, `re-terrain-lighting.md` §4) when `DAT_005229bc` != 0 (113440-113443); tint = min(1.2·tint + 0.3, 1)
  (113446-113447); the draw colour = (tint, tint, tint)·255 with the fade alpha (113567-113577, 114046-114059). It only
  darkens: a soldier on LUMI under 0.583 is drawn at down to 0.3.
- `DAT_005229bc`: 0 at the start of BF1's `DomeInfo` loader `FUN_002e29e0` (270061), 1 on the valueless key 0xd8a93a22 =
  FNV-1a("TerrainColorDarkening") (270363) [receipt:4e7a6368] (the name found by the verifier, the hash recomputed here;
  the same loader's 0xae291b6d is `TerrainEnable`; the loader is reached under `DomeInfo`, 0x185ab6c2, 271557). Disc: the
  key is in END1, NAB1(+_H), TAT1(+H), TAT2; RAM: the flag is 1 on END1, NAB1, NAB1h, TAT2 (`sblt/ramcensus.txt`).
- B2 twin `FUN_00239bb0` (153625-153631): the same 1.2 / 0.3 / clamp, gated by `DAT_0060bed4` (`FUN_0055db58` = 1,
  `FUN_0055eb50` = 0; read in 20+ places, [H] "terrain loaded") AND `DAT_0060bedc` (BF2's sky dome loader
  `FUN_003bcd50`: 0 at 416184, 1 at 416290, reached from `TerrainColorDarkening` 0xd8a93a22 (416392) and from 0xed4a20e6
  (416285)); both 1 only on BF2 END1 and KAS2 (RAM).
- The port draws every soldier at colour 1.0 (`soldier_vis.rs`, `SkinnedDrawCmd::color`). Card SLUMI.

## 5. Measured (the card's acceptance: name the term) [V-PS2]

PS2 BF1 KAS2 dock two, SBD, no pass, one mask (`sblt/measure_tables.txt`, frames `sblt/ps2/frames/kas2_*`, sheet
`sblt/sheet_kas2_terms.png`), HW renderer; every patch read back from the saved state:

| variant | PS2 | port, same knob |
|---|---|---|
| as is (0x578 = 128) | 51.2 (ctl2 51.3) | 28.1 (port's AmbientColor) |
| 0x578 = AmbientColor (the port's value) | 38.8 | 28.1 |
| 0x578 = 128 | 51.2 | **43.8** |
| 0x578 = 64 / 255 | 32.6 / 92.8 | |
| ambient 128 only (sun 0) | 36.2 | 34.5 |
| sun only | 15.6 | 4.8 |
| omnis off | 51.4 | |
| all off | 3.3 (the halo) | 0 |

## 6. Checks, the verifier, what stays [H]

- **Checks run.** `re_check.py fn` on every cited line; `absent` [receipt:f90cab11] (0x69c578), [receipt:bb3a1463]
  (`FUN_00493670` / `FUN_004790e0` and their address words, `--elf`), [receipt:4e7a6368] (`DAT_00522a88`,
  `DAT_005229bc`), [receipt:b857caf5] (0xcda12257 both forms in B2, `--elf`); `writers` for `DAT_00522a88`,
  `DAT_00522a8c`, 0x69c578, `DAT_005229bc`, `DAT_0060bed4`, `DAT_0060bedc`; `word` 0x527350 = 'MISSION.lvl'; `callers` for
  `FUN_00493670`, `FUN_004790e0`, `FUN_00173a98`, `FUN_004b8e78`, `FUN_00479458`, `FUN_00493328`, `FUN_00494d18` (the static
  initialiser: called by `FUN_00496028` / `FUN_00496048`, the first held once as an ELF data word, the constructor
  table [receipt:d58f58ee]), the LUMI lookups. Twin: B2 `FUN_00239bb0` and its flags' writers.
- **Verifier** (workflow `sblt-re-verify`, 5 agents at medium effort, given the claims and citations only): 25 claims -
  23 SUPPORTED, 2 PARTLY, 0 NOT. The six absence / twin claims went to two readers each: both SUPPORTED X1-X5; X6 (BF2's
  two flags) one SUPPORTED, one PARTLY (the writers right; `DAT_0060bedc` is reached from two keys - folded in). R1 (the
  RAM census) PARTLY: KAM1's lightning rows - folded in (§1.5). Precisions folded in: A4's line numbers, A5's per-view
  gate and third caller, X2's whole-slot initialiser. The verifier named 0xd8a93a22 (`TerrainColorDarkening`), checked
  here by recomputing the FNV-1a. No claim overturned by the second reader.
- **Stays [H]:** the load order that makes the first frame precede the sky's parse (§1.4; the RAM settles the outcome on
  every non-CAMB map's first mission after boot; a SECOND mission after a CAMB map was not sampled - the reset per
  mission start makes it 128 too unless a frame is skipped); the sun gap's cause (§3.4); BF2 `DAT_0060bed4`'s meaning.

## 7. Reproduction

`sblt/tools/`: `b1pose.py`, `b2pose.py` (pose), `ramcensus.py`, `disckeys.py` (data), `measure_fixed.py` (one-mask
luma), `port_kas2.sh` / `port_kam1.sh` + the scratch example `sblt_soldier.rs` (in the detached worktree
`<repo parent>/openbf-wt/sblt-scratch`, target `lead_tmp/sblt_target`; env `SBLT_CAMB`, `SBLT_NOSUN`, `SBLT_NOAMB`,
`SBLT_SUNT`, `SBLT_SUNS`, `SBLT_CLIP`, `SBLT_TINT`); `sblt/ps2/mk1.py` + `run.py` (PCSX2 patched states).
