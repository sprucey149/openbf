# OPUS RE (2026-10-22) - [O] UNAUDITED
# The weapon heat gauge on the HUD, BF2 and BF1 (card OVHT)

Corpora: B2 = `analysis/SWBFRNT2.ELF-decomp.c`, B1 = `analysis/SWBFRONT.ELF-decomp.c`; DUMP = BF2's HUD layouts as text
(`engine/artifacts/lead_tmp/re/bhud/dump_named.txt`, card BHUD). Tools and captures: `engine/artifacts/lead_tmp/ovht/`
(`tools/evtable.py` names BF2's HUD event table in a save state, `tools/heatcensus.py` the heat weapon census,
`tools/ringarc.py` the ring's lit arc in a frame; `ps2/` the live runs; `checks/` the re_check outputs and the
verifier table). Tags: [V] read (lines cited), [D] derived, [H] hypothesis, [M] measured on the PS2 (RAM or frame).
Hops: (1) BHUD's layout dump for the heat elements; (2) the BF2 publisher `FUN_003b3398` and its event table, named from
RAM by FNV; (3) BF2's heat component (spend, refill, keys, defaults) and the weapon states; (4) BF1's weapon update and
its code-drawn weapon panel `FUN_002c0400`; (5) live PCSX2 runs, both games: the pistol tapped to overheat, the heat
read every update, five frames each; (6) a census of heat weapons on both discs' bakes; (7) the port's HUD feed;
(8) re_check fn / word / callers and the receipted searches; (9) workflow `ovht-re-verify` (section 8).

## Headline
- **BF2** [V][M][O]: the ring is `player1weapon1heatbar`, 30 `white` segments from -150 to +150 degrees (0 = up,
  clockwise) around the reticle, inside the reticle group. The game sends `player1.weapon1.heat` EVERY frame; for a
  weapon with unlimited rounds (RoundsPerClip authored under 1) it is the heat, 0..1, so the ring shows k of 30
  segments while k/30 < heat (none when cold). Its colour follows the heat: yellow (239,255,41) at 0, (239,174,41) at
  0.5, orange (239,106,41) at 1. While the weapon is overheated the game also sends `player1.weaponsOverheat` (the line
  "OVERHEATED!", orange (230,70,0), pulsing, centred at y 0.75) and a `refire` value 1 - HeatThreshold/heat that lights
  two small red 3-segment bits under the reticle (6 o'clock) shrinking to nothing at the clear mark. A COUNTED weapon
  (the bulldog, the sonic blaster) shows no heat on the ring - only the line and the red bits when it overheats; a
  counted CHARGE weapon fills the same ring with its charge while the trigger is held.
- **BF1** [V][M][O]: a different element - a horizontal bar on sheet `hud1` in the weapon panel at top left, in the
  place of the ammo count, shown only for a weapon with no ammo per shot: full and blue (0,152,255) when cold, its
  width 1 - heat, red (200,100,100) while overheated. No text, no ring.
- **Heat** both games [V][M]: +HeatPerShot a shot, -HeatRecoverRate x dt every update, overheated past 1.0, cleared at
  heat <= HeatThreshold; the pistol (0.175 / 0.3 / 0.25) overheats after 6 shots at its 0.2 s ShotDelay, on the 11th
  at the live runs' one tap per 0.27 s, and clears about 2.6 s later. Verified per update on both PS2s (section 6).
- **The port** [V]: the sim already keeps `WeaponState::heat` / `overheated` (BF1's rule, for hand weapons and seat
  guns); the HUD feed sends none of the three events [receipt:416fa2b8], and off foot it shows no reticle at all.

## 1. BF2's elements (layout data) [V][O]
- Ring (DUMP:1580-1601): `barsegmented("player1weapon1heatbar")` anglestart -150, angleend 150, numsegments 30,
  value(1) (until the first event), eventvalue + eventenable `player1.weapon1.heat`, eventdisable
  `player1.weapon1.disable`, eventcolor `player1.weapon1.heatcolor`, Rect 0.092845 x 0.120721 Center Center Viewport,
  propagatealpha 1, alpha 0.6, color (239,255,41); segment `Bitmap("white")` Shadow, bitmaprect 0.016252 x 0.012504,
  alpha 0.75, fadein 0.1, fadeout 0.3. It sits in `Group("player1reticule")` (DUMP:1479, alpha 0.65, enabled by
  `.weapon1.reticule.position`, disabled by `.weapon1.reticule.disable`), next to the clip ring.
- Colour (DUMP:98-104): `transformnumbercolor("player1weapon1heatcolor")` 0 -> (239,255,41), 0.5 -> (239,174,41),
  1 -> (239,106,41), from `player1.weapon1.heat` to `player1.weapon1.heatcolor`.
- Refire bits (DUMP:1602-1643): `player1weapon1refire_leftbit` and `_rightbit`, each anglestart -180, angleend -140,
  3 segments, value / enable `player1.weapon1.refire`, disable `.weapon1.disable`, the same Rect, alpha 0.6, red
  (255,0,0), segment `white` Shadow 0.007862 x 0.012504, fadein 0.05, fadeout 0.1; the right one at Position
  (0.092858, 0.001168) with rotation (0,180,0) - the mirror image, so the bits grow from 6 o'clock outward both ways.
- Line (DUMP:1645-1659, top level, not in the reticle group): `Text("player1weapon1overheat")` key
  `game.weaponOverheat` = "OVERHEATED!" (BF2 English locl 3724984183, `common_bf2/locl/english.ron`), gamefont_medium,
  Center/Top at (0.5, 0.75), Shadow, word break, z 5, colour (230,70,0), colorpulserate 0.75, fade in 0.25 / hold 0.1 /
  out 1, enabled by `player1.weaponsOverheat`, disabled by `player1.weaponsDisable`.
- The `lockon` transforms (DUMP:105-140) also read `.weapon1.heat` (chargeScale / chargeRotate / chargeColor) for the
  lock-on group, which shows only with a lock-on position; not this card.

## 2. Who sends what (BF2 `FUN_003b3398`) [V][M][O]
- [M] The publisher's event table (`tools/evtable.py`, `bf2_cor1_cp1_cp2_postA_spawn.p2s`): RAM 0x6a91f8 + player x
  0x31c holds pointers to event objects {fnv(name), type}; the weapon blocks (argument 7) at +0x68 + w x 0x68: [7] =
  `player1.weapon1.heat`, [8] = `.charge`, [9] = `.refire`, [1] = `.disable`, [5] = `.totalClipFraction`; the player
  block +0x138 = `player1.weaponsOverheat`.
- [V] `FUN_003b61e0` calls `FUN_003b3398` for weapons 0 and 1 of each local player (B2:411749, argument 6 = the player
  block, argument 7 = its weapon block); `FUN_00425840` calls `FUN_003b61e0(DAT_0066137c)` in its frame (B2:482586,
  482639) - one aligned jal each way in the ELF [receipt:63113c6c]. Sends go `FUN_003b0538` -> `FUN_003b0228`, which
  calls every listener with no compare against the last value.
- Unlimited test (B2:409823, 409900-409906): class+0x98 >= 2^31 or class+0x9c == 0 - the RoundsPerClip component
  (`FUN_003035d0`, B2:294432-294443: authored < 1 -> 0x7fffffff and 0, else n and 1/(n + 0.0001)). The OWNER's reserve
  (WeaponAmmo) does not enter it.
- `heat` (argument 7[7]), every call: unlimited -> clamp(1 - cur / max, 0, 1) of the heat component (B2:410010-410019);
  counted -> 0, or in weapon state 3 (charging) +0xbc / +0xc0 = time held / MaxPressedTime (B2:409966-409980,
  410028-410031).
- State 5 (overheated): if heat > 1 - thr/max it sends `weaponsOverheat` and sets refire = 1 - clamp((max - thr) /
  (max - cur)) = 1 - HeatThreshold / heat at max 1 (B2:409993-410008). `refire` (argument 7[9]) is sent every call
  (B2:410034); an unlimited weapon's refire is forced to 0 while heat is under the clear mark (B2:410020-410025); a
  counted weapon's refire also carries state 4 / single-shot progress (1 - +0xbc/+0xc0, B2:409971-409992) - BHUD3's.
- The weapon states (+0xb8): 0 idle, 1-2 firing ((class+0x1e6 & 3) + 1, B2:318157), 3 charging (vtable +0xd8 =
  `FUN_0032ee30`: MaxPressedTime class+0x240 > 0 and the trigger down; +0xb4 `FUN_0032ed68` / +0xb8 `FUN_0032ed98` run
  +0xbc up to +0xc0 = MaxPressedTime; B2:318160-318172, 323660-323724; the vtable 0x628350 slots read in RAM [M]),
  5 overheated (`FUN_003269f8` / `FUN_00326b28` set 5 when `FUN_00327820` = the component's bit 0, else 0;
  B2:318412-318480, 319048-319075). [M] live: 1 after a shot, 0 idle, 5 while the bit is set (`ps2/bf2_pistol/rep0.csv`;
  the note "6 overheated" in an older sampler legend is wrong for this weapon).

## 3. BF2's heat (the component at weapon+0x94) [V][O]
- Built by the weapon ctor `FUN_00325448`: 12 bytes {class+0xa0, cur = max, flags 2} (or a shared one)
  (B2:317542-317556). Class data at +0xa0 = {max, min, thr}: ctor `FUN_00327e98` -> `FUN_001e1c88(1.0, 1.0)` = max 1,
  thr 1 (B2:319542, 92428-92442); `FUN_001e1cb8` (from the weapon setter `FUN_00328fb0`, B2:320268): EnergyBar
  0x97105d6a -> max; HeatThreshold 0x5621511e -> thr = 1 - v (0 < v < 1), 0 (v >= 1), 1 (v <= 0) (B2:92445-92497);
  `FUN_0032aef8` -> min = -(the largest HeatPerShot up the class chain) (B2:321046-321066, `FUN_001e1e28`).
- Spend `FUN_001e2010` (fire cycle `FUN_0032e8c0`, HeatPerShot class+0x90, B2:323587): refused while bit 0 is set;
  cur -= cost; cur < 0 -> bit 0 set, cur floored at min (B2:92617-92637). Refill `FUN_001e1fc0` (weapon update
  `FUN_00326170`, HeatRecoverRate class+0x94 x dt, every update, B2:318140): cur += a; cur >= thr -> bit 0 cleared,
  cur capped at max (B2:92597-92612). So heat = 1 - cur: overheated once a shot takes it past 1, cleared at heat <=
  HeatThreshold, the shot that crosses 1 still fires.
- [D] census: EnergyBar is unset on every heat weapon class of both bakes (`tools/heatcensus.py`; the 40 BF2 classes
  that author it are entity classes - soldier and vehicle energy), so max = 1 and the ring's heat is 1 - cur.
  (Verifiers: my first wording, "no class authors EnergyBar", was refuted by both - see section 8 [receipt:fba8254c].)
- [V] `FUN_00327820` returns 0 for an AI owner whose controllable answers the type DAT_00689368 (the weapon then never
  reads as state 5, though the spend still refuses) - not a HUD case.

## 4. BF1 [V][M][O]
- Heat, weapon update `FUN_00284318`: the flag +200 latches to (heat +0x9c >= 1.0) while clear (B1:208434-208436);
  heat = max(heat - dt x HeatRecoverRate class+0x6c, 0) (B1:208441-208443); while set it stays set while
  HeatThreshold class+0x70 < heat (B1:208444-208446). Class defaults `FUN_00285a98`: +0x64 (ammo per shot as a clip
  fraction) 1.0, +0x70 1.0 (B1:209498-209499); RoundsPerClip < 1 -> +0x64 = 0, +0x60 = 0x7fffffff (B1:210084-210095);
  AmmoPerShot v -> +0x64 = v / (RoundsPerClip + 0.0001) (B1:209927-209936).
- Element, `FUN_002c0400` (one call per slot from `FUN_002c0f18`, B1:251135-251136, two aligned jal [receipt:63113c6c]):
  in weapon state 4 the bar +0x620 shows +0xd0/+0xd4 in green DAT_0069c450 (B1:250764-250782; [H] that state 4 is
  BF1's charge). Else, for class+0x64 == 0 (B1:250785) the counts hide and, while the weapon icon is shown, the bar
  shows width x max(1 - heat, 0) with its u range cut alike, colour 0xffff9800 = (0,152,255) when the flag is clear,
  DAT_0069c448 when set (B1:250797-250815). A weapon with ammo shows no bar outside state 4.
- Layout: slot ctor `FUN_002bfe18` (from `FUN_002c0be0` with tables 0x518508 / 0x518550, B1:250963-250964) and loader
  `FUN_002c0178` (B1:250664-250668): sheet "hud1" (0x53fb30); primary bar at (30, 2) size 84 x 13, uv (0.828125,
  0.0390625)-(0.171875, 0.140625); secondary at (32, 2) 48 x 12 (`re_check word`).
- [M] RAM (`ps2/bf1/s2_r2.p2s`, .bss): DAT_0069c448 = (200,100,100,255), DAT_0069c450 = (100,200,100,255).
- BF1's English locl has no overheat line; its one heat string is "THIS METER SHOWS WEAPON HEAT LEVEL" (help text).

## 5. Census (`tools/heatcensus.py`, `heatcensus.txt`) [D]
- BF1: 132 weapon classes, 56 with HeatPerShot > 0, all named by a unit or vehicle; 55 unlimited by the PS2's rule (17
  infantry: pistols, commando pistols, remote droid blasters, fusion cutters, the droideka repeater, wrist shrapnel,
  the arc caster; 38 seat guns), 1 counted (`all_weap_hover_hovernaut_cannon1`, RoundsPerClip unset = 1).
- BF2: 292 weapon classes, 75 with heat; 69 unlimited (27 infantry, 42 seat), 6 counted: cis/com bulldog (6 rounds),
  imp/com sonic blaster (7), `tur_weap_spa_beam` (1), `all_weap_inf_invisibility` (unset).
- Every heat class authors HeatThreshold and HeatRecoverRate, so neither game's defaults (BF1 threshold 1.0, BF2 thr 1)
  reach a fielded weapon. The port reads RoundsPerClip unset as unlimited (`weapon.rs` default 0): it differs from the
  PS2 on two heat classes (the hovernaut cannon, the cloak).
- Counted charge weapons (MaxPressedTime > 0) that would fill BF2's ring while charged: bowcaster, mortar launcher,
  arc caster (BF2), detpack, remote-droid destruct, the Hoth dish turret.

## 6. Live PS2 (PCSX2, SW renderer, `padscript.py`, R1 tapped 3 updates on / 5 off) [M][O]
- BF2 KAM1 clone, blaster pistol (`ps2/run_bf2_pistol.sh`, from HUDW's `s3_r2_once.p2s`; class RAM: max 1, min
  -0.175, thr 0.75, HeatPerShot 0.175, HeatRecoverRate 0.3): +0.175 a shot, -0.01 an update; bit 0 set at the 11th tap
  (u99, cur -0.045, heat 1.045), state 5 from u106, taps refused, cleared at cur 0.755 (u179, heat 0.245 <= 0.25).
  Frames (`ps2/bf2_pistol/rep0_{half,over,cool1,cool2,cold}.png`, `strip_ring.png`): lit arc (`tools/ringarc.py`)
  0.598 / 0.627 / 0.437 / 0.06 / none against the logged heat over each grab's window 0.59-0.50 / 0.59-0.50 /
  0.39-0.31 / 0.02-0 / 0 - within a segment plus the 0.3 s segment fade-out and the grab lag; red bits at 6 o'clock in
  the two overheated frames only; "OVERHEATED!" in orange in the two overheated frames only; ring yellow (189,188,76)
  near 0, orange (174,131,54) at 0.5-0.6.
- BF1 NAB2 rebel, blaster pistol (`ps2/run_bf1_pistol.sh`, state `ps2/bf1/s2_r2.p2s` made by `ps2/mk_bf1_pistol.sh`;
  R2 cycles the primary): +0.175 / -0.01; the flag latched the update after heat reached 1.034 (u100 -> u101), state 7
  while set, cleared at heat 0.243 (u179). Bar fill on the five frames 0.407 / 0.434 / 0.690 / 1 / 1 against 1 - heat
  0.41-0.51 / 0.44-0.52 / 0.63-0.72 / 1 / 1; red in the two overheated frames; no text, the reticle unchanged.
- In a BF2 seat (Mygeeto turret, `lead_tmp/vturz/pairs/myg1_zoom_1_rest__pair.png`) the PS2 shows the reticle group
  and the weapon panel ("PARTICLE CANNON", infinite): the ring belongs on seat guns too.

## 7. The port, and what to build (decisions are reversible; PLAN puts BF2's HUD on every map)
- Has [V]: `core::weapon::WeaponState::heat` / `overheated` (BF1's rule: latch at >= 1, clear at <= threshold - the
  same numbers as BF2's to the update on the pistol), `WeaponClass::heat_threshold`, seat guns on the same state
  (`sim/guns.rs`); the BF2 tree's BarSegmented draw (`hud_draw::segmented`, `FUN_003a04b8`) and NumberColor
  transforms (`hud_tree`). Lacks: the three events [receipt:416fa2b8]; any reticle or weapon panel off foot
  (`hud_model` gates both on `on_foot`).
- Build (card OVHT): each frame send `player1.weapon1.heat` = clamp(heat, 0, 1) for an unlimited weapon (the port's
  `infinite_ammo()`), else 0; `player1.weapon1.refire` = 1 - clamp(threshold / heat) while overheated, else 0; and
  `player1.weaponsOverheat` while overheated - the PS2's every-frame sends, so the line holds 0.1 s and fades 1 s
  after the clear. BF1 maps get the same BF2 ring from BF1's heat (PLAN's one HUD), a deliberate deviation from the
  PS2 BF1's bar (section 4).
- Not this card: the charge on the ring (counted charge weapons) and the state-4 refire bits (BHUD3's); the reticle
  and weapon panel in seats (new card VSHUD) - OVHT's seat half rides on it.

## 7.1 What the port does (issue #78, 2026-11-01)
- `game::hud_feed::weapon_gauge` reads the sim's state, on foot, in a droid and in any seat; `ui::hud::feed` sends
  section 7's three events every frame. A counted charge weapon fills the same ring while held, as section 2 reads
  (`WeaponState::charging`); this goes past OVHT's on-foot slice and takes the charge half from BHUD3's.
- `hud_draw::segmented` turns a bar about its rect's corner. Turned about its centre, as it drew before, the layout's
  `refire_rightbit` stood one rect-width to the right of the reticle rather than meeting the left bit at 6 o'clock.
- The port deliberately uses BF2's ring on BF1 maps: the ring around the reticle, the red bits and the "OVERHEATED!"
  line, fed by BF1's heat and charge (the maintainer's decision, PLAN's `HUD: BF2`). The PS2 BF1 does otherwise (section
  4: a bar in each weapon row, no ring, no text). That stays the record of the original and is not to be built.
- The bake holds the layout on the BF2 disc only; `tests/suite/hud_layout.rs` and `tests/suite/weapon_gauge.rs` test
  it. Compared with the PS2's frames by eye and by the red bits' bounds (`lead_tmp/ovht/ps2/bf2_pistol/rep0_over.png`,
  `rep0_cool2.png` beside `hud_capture` at the same heat): the lit arc starts and runs the same way, the colours match
  and the bits sit centred under the reticle in both. The two frames do not share a scale, so this is not a pixel match.
- Not built: a counted weapon's single-shot `refire` (state 4, BHUD3's); the ring on `weapon2`, which BF2's layout does
  not have, so a charge on the secondary channel (the detpack) shows nothing, as there.

## 8. Checks run and the verification (`checks/`)
1. `re_check fn` on 64 cited lines (`checks/fn.txt`, `word_callers.txt`): each sits in the function named (e.g.
   *(decompiler detail omitted)*, *(decompiler detail omitted)*, *(decompiler detail omitted)*).
2. Absence: `re_check absent` - the port's ui/game sources hold none of `"heat"`, `"refire"`, `Overheat`, control
   `"totalClipFraction"` 1 hit [receipt:416fa2b8]; ELF jal words 0x0c0ecce6 (1, B2), 0x0c0ed878 (2, B2), 0x0c0b0100
   (2 aligned + 1 unaligned byte match, B1) [receipt:63113c6c]; EnergyBar's tuple head in the class tables
   [receipt:fba8254c] - 56 hits, which the first draft misread as none (corrected in section 3).
3. Twin: BF1 is not BF2's element - its own function `FUN_002c0400` and its callers are cited above.
4. Words: `re_check word b1 0x53fb30 --str` = "hud1"; 0x518508 / 0x518550 tables (section 4); RAM colours [M].
5. Writers: the overheat bit's writers read for this doc are the spend / refill / set helpers `FUN_001e2010`,
   `FUN_001e1fc0`, `FUN_001e1f48` and the ctor's flags 2 (bit clear); BF1's flag +200 is written by the update at
   B1:208435 and 208445 (not an exhaustive writer list; both rules were checked per update on the PS2, section 6).
- Workflow `ovht-re-verify` (5 agents, medium effort, claims and citations only): 16 claims in three batches, one
  verifier each - 16 SUPPORTED (R8 with the charge path's extra conditions noted: the +0xd8 test runs only under the
  weapon's +0xb0 bit 34 and after the +0xdc path); 4 absence / only-caller claims to two separate verifiers each - 3
  SUPPORTED by both, 1 (X2, "no class authors EnergyBar") NOT SUPPORTED by both [receipt:fba8254c]: fixed to
  section 3's census claim ([D], not [O]). 24 verdicts: 22 SUPPORTED, 2 NOT SUPPORTED (one claim); the second
  verifier overturned nothing the first had passed.
- Stays [H]: BF1's state 4 = charge; the BF2 Text fader's exact hold after the last event (the port's `hud_tree`
  fader already models FadeIn / Hold / Out).
