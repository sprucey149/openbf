# OPUS RE (2026-10-31) - [O] UNAUDITED

# RE: the first-person arms and weapon (BF1 + BF2) — the FPM model, its clip table, its placement, its weapon

Card FPA. Builds on `re-first-person.md` (the eye, the toggle, §5 "what is drawn"). Corpora: B1 `SWBFRONT.ELF-decomp.c`,
B2 `SWBFRNT2.ELF-decomp.c`; the static tables were read from the ELFs (`SWBFRONT.ELF` / `SWBFRNT2.ELF` off the discs).
Tags: [V] read, [D] derived, [M] measured on disc data, [H] hypothesis. No live (PCSX2) check.

## 0. Headline

| | BF1 | BF2 |
|---|---|---|
| record | per player, `DAT_0057f730 + i·0x420` | `DAT_00682620 + i·0x430` |
| clip choice | `FUN_001a38e0` (B1:47224-47462) | `FUN_001bfcb0` (B2:67155-67385) |
| playback + placement | `FUN_001a2bc8` (B1:46464-); `FUN_001a30c0` the same for the second path | `FUN_001befd0` (B2:66398-) |
| draw | `FUN_001a4250` (B1:47708-47774) | (its twin, not read) |
| bank | `human_animation_fp` (COMMON/ENG/COMMON.LVL) | `humanfp` (INGAME.LVL) |
| table | 5 rows × 11 columns at `DAT_00581510` | 4 rows × 11 at `DAT_00684438` |
| model offset | T(0, 0.35, −0.3) · Rz(0.05) · Ry(3.0) | T(0, 0.35, 0.05) · Rz(0.05) · Ry(3.0) |

## 1. The model [V][M]

- Class key `FirstPerson` (0x6363d774) = `<FPM lvl>;<model>`, e.g. `IMP\impstrp;imp_1st_stormtrooper`. The part after `;`
  names the `modl` in `FPM/<side>/<lvl>.LVL`. Vehicle classes name cockpits the same way (`IMP\ATST;imp_1st_cockpit_atst`).
- The arm rig [M]: `dummyroot`, `bone_shoulders`, `bone_l_forearm`, `bone_l_wrist`, `bone_r_forearm`, `bone_r_wrist`,
  `hp_weapons` (child of the right wrist) and, in `imp_1st_stormtrooper` only, `hp_camera` (child of dummyroot, bind
  (0, −0.22, −0.4145), no rotation). The model is rigid per bone: its `modl` INFO bone-space word is clear, yet every
  vertex sits in its BNAM bone's own frame (within a few cm of the origin) [M], and the draw hands the model the posed
  bone table (B1:47748-47750) [V]. Ours: the bake lifts the vertices by the bind pose as for a bone-space model.
- The clips carry 9 tracks: DummyRoot twice, the arm bones, `hp_weapons` and `hp_camera`. The `hp_camera` track is the
  constant (0, −0.22, −0.4145), identity, in every clip that carries it (the lascannon row's clips carry 8 tracks and no
  camera) [M]. The engine looks `hp_camera` up in the POSED bone table (`FUN_00444998(table, 0x100, crc)` after
  `FUN_00498040` fills it from the animator's bone list at +0x830, B1:46563-46568, 558565-558632) with no null test, so
  every soldier's camera bone comes from the clip side, not the model's `skel` [D]. Ours: the bake adds the bone to a
  rig without one, from the first table clip's track.
- DummyRoot's clip translation (0, 0.704, 0.108) is not used: the rig's own (0, 0, 0) is [D, by the placement below:
  with it the shoulders sit 0.12 m under and 0.6 m in front of the eye; with the clip's they sit 1.3 m above it].

## 2. The clip table [V]

`DAT_00581510` (B1) / `DAT_00684438` (B2) point at 11 clip-name CRCs per row. Columns: 0 idle, 1 run, 2 shoot,
3 shoot2, 4 charge, 5 reload, 6 repair, 7 jump, 8 flail, 9 handsdown, 10 jump land. Rows and names in
`engine/crates/manifest/src/fpm.rs` (`BF1_TABLE`: rifle, lascannon, bazooka, tool, grenade; `BF2_TABLE`: rifle,
bazooka, tool, grenade; slots the disc leaves empty name `_1st_tool_idle` / `humanfp_tool_idle`).

**The row** [V]:
- BF1 switches on the weapon's set (weapon+0xec = class+0x20, `re-soldier-anim-layers.md` §4): 0 rifle and 7 bdroid → 0;
  8 (lascannon) → 1; 1 bazooka → 2; 3 grenade → 4; any other (pistol, tool, lightsabre, sbdroid) → 3.
- BF2 `FUN_001beeb8` (B2:66339-66361): `OffhandWeapon` (class+0x1e0 bit 55) → 3; else the weapon's animation class
  (class+0x20, walked to its base row by `FUN_00278820`, rows `rifle bazooka tool pistol melee`): rifle → 0, bazooka → 1,
  anything else (or none, −1) → 2.

## 3. The clip choice [V]

Each update, for the weapon in hand (BF1: the active channel's while its state is 1..3, else channel 0's; BF2: states
1..2), with speed² = the body's velocity squared and the stance code (BF1 soldier+0xbe4, BF2 +0x7e4):

1. While the column is 2 or 3 (shoot), the clip has not ended (+0x15fc) and the weapon has no new shot flag (B1
   weapon+0xc0, B2 bit 33 of +0xb0): keep playing.
2. **Weapon change** without a shot flag: play handsdown (column 9, blended) and skip 3-4; once in column 9 with the
   blend weight +0x1610 at 0, adopt the weapon and go on. With a shot flag, adopt at once.
3. **Fire**: state 1..3 (BF2 1..2) with the shot flag → column 2 for state 1, 3 otherwise; clear the flag; restart
   with no blend; not looped. BF2: a weapon class with bit 60 of +0x1e0 takes column 9 instead. Reload (BF1 state 5,
   BF2 4) → column 5, not looped.
4. **Locomotion** otherwise, by stance: jump (4) → column 7 and a pending column 10 for 0.5 s; fall (BF1 6, BF2 8) →
   column 8 looped and a pending column 9 for 1.0 s; jet (BF1 7, BF2 6/7) → keep; roll (BF1 10/11, BF2 5) → column 7,
   not looped; any other → column 1 looped, or column 0 while speed² < 0.1.
5. Moving prone (stance 2, speed² ≥ 0.1) → column 9 looped, no pending.
6. **Pending**: unless 4 or 5 set it this update, while timer < duration the column is the pending one, timer += dt.
7. **Spawn** (B1 +0x16d0, B2 +0x16c4, set by the reset): column 9 with no blend, then pending column 0 for 1.0 s.
8. A changed column restarts the clip (`FUN_001a3858`: 30 fps, frame 0, the loop flag) and sets the blend weight b to
   1, or 0 for the shot and spawn snaps. BF2 sets b = 0 while the model was never posed (+0x16b4).

The reset (B1:47062-47080) leaves row 5 and column 11, out of range, so the first choice always restarts.

## 4. Playback [V]

B1 `FUN_001a2bc8` head (B1:46535-46560), BF2 `FUN_001befd0` alike:
- rate = 1, but in column 1: (speed² ≤ 20 ? speed² · 0.05 : 1) · 0.75; the clip advances rate · dt (`FUN_0043e158`).
- While b > 0: the pose (+0x830) moves toward the clip's frame by (1 − b)² (`FUN_0043ff58`), b −= 3 · dt; then the
  clip's frame is taken whole (`FUN_0043fa98`).

## 5. Placement [V][D]

Row-vector matrices (`v' = v · M`) on the PS2; in column form:

    arms  = Cam · H · T(0, 0.35, z) · Rz(0.05) · Ry(3.0)          z = −0.3 BF1, +0.05 BF2
    gun   = arms · W

- Cam: the eye camera (`re-first-person.md` §1), looking along −Z; `FUN_001a53c8` copies it to `DAT_00580c70`.
- H: the posed model-space `hp_camera`; W: the posed model-space `hp_weapons`.
- The offset: translation `DAT_0057f620/624/628` (B1, set at B1:49630-49632 in `FUN_001a6880`) / `DAT_005fc53c-544` (B2);
  angles `DAT_00517450` = 3.0, `DAT_00517454` = 0.05 (B2 0x5fc550 / 0x5fc554), applied by `FUN_004462d8` (Y) and
  `FUN_00446378` (Z), each M' = M · R, sines from `FUN_00445208`.
- The order was settled numerically [D]: of the five candidate products only this one puts the hands below and in
  front of the eye with the barrel (`hp_weapons` +Z) along the view, 0.036 off-axis.
- No projection of its own [V]: the draw `FUN_001a4250` (B1:47708-47774, called from `FUN_001a4938`) hands the arms
  and the gun the scene's camera; `0x69c420` is the draw colour. So the arms fill the screen as the first-person FOV
  frames them: BF1's 48 deg (no soldier authors `FirstPersonFOV`), vertical tan 0.34131; BF2's 70 deg, 0.53678
  (`re-zoom.md` §5). At BF1's frame the forearms' cut ends fall just below the screen [M: a BF1 Kamino capture
  cropped to it]; a wider view shows them.
- Arm lag: class +0x1b0 (hash 0x104cd1b8) slerps the matrix by exp(−k · dt) (B1:46593-46612). Only vehicle classes
  set it (12-40) [M]; soldiers have none.

## 6. The weapon [V]

- The draw `FUN_001a4250` (B1:47736-47774) runs only for the player's own view: the arms model (+0x16ac) at +0x1620 with the posed
  bone table, then the weapon instance in hand (+0x16b0) through its own vtable slot (+0x14 → +0x64) at +0x1660.
- Its model is the weapon class's `HighResGeometry` (0x8b3185dd, B1 class+0x50, B2 +0x78): `*_1st_weap_inf_*`,
  `com_1st_weap_inf_*`, each with an `hp_fire` node [M]. What that slot draws for a weapon without one is not traced
  [H: nothing]; every soldier weapon on both discs names one [M].
- The muzzle flash and the drawn bolt leave the weapon's fire point on that model [D: the flash rides the drawn weapon,
  `re-muzzle-flash.md` §5].

## 7. Not traced

- The view animations (camera clips RollLeft, RollRight, Crouch, CrouchToProne, StandToProne, ProneToStand,
  CrouchToStand: `FUN_001911a0`, `FUN_001a5608`, `FUN_001a5df0`, `FUN_001a6330`, class+0x10) and the walk bob
  (`FUN_001a5b80` start at speed² ≥ 1 with no view animation, rate 0.5 prone; `FUN_001a5aa0` stop when idle).
- BF2's bit 60 of weapon class +0x1e0 (column 9 on fire): which key sets it.
- The engine's dt globals: B1 `DAT_005644e0` (playback, blend) and `DAT_005644e4` (pending timer, lag) are both taken
  as the frame's dt.
- The draw's depth handling (cleared depth, own pass): not read; ours draws the arms in the world pass.

## 8. The port (2026-10-31)

Built: `bake fpm` (`bake::fpm`, every FPM LVL's arm model with its table clips, the camera bone added where the rig
lacks it; cockpits and the droideka skipped) and the 1st-person weapons by `bake weapon` (`scripts/rebake.sh` side
group); `game::roster::first_person_model` / `first_person_weapon` (model, `HighResGeometry`, row);
`game::first_person` (§2 rows, §3 `Chooser`, §4 `Playback`, §5 `model_matrix` from any eye pose); the draw
`SoldierVisuals::first_person_draws` (windowed and headless), shown while `first_person_view()` and unzoomed; the
player's fire point (flash and drawn bolt) at the weapon's `hp_fire`. OURS (2026-10-31, the user's choice): the
first-person camera keeps the port's FOV (BF2's 70 deg on every map, times the FOV option), so `model_matrix` scales
the arms and the gun about the eye by k = tan(fov_y / 2) / the game's PS2 tan in eye x and y: each point lands on the
screen where the PS2's frame put it (`frame_scale`; k = 1 on BF2 at the default option). The muzzle rides the same
matrix. The shader takes normals through the model matrix, so the arms' shading leans slightly with k. Not built: §7, cockpits, BF2's bit 60, the
state 2/3 shoot2 column (a shot plays column 2).
