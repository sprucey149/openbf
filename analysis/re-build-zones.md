# OPUS RE (loop lane 2, 2026-10-03, card TBLD) - [O] UNAUDITED: no Fable pass
# RE: how a build pad becomes its turret - the building's build state, the fusion cutter, the bots that build, the hologram

**The question** (card TBLD). BF2's buildable turrets start as flat pads (card VTUR2) and the PS2's bots turn them
into turrets within seconds; the port never does. What fills a pad's health, who may do it and how fast, which bots
go to a pad and when, and who sees the green hologram over it? All four are answered for both games, and the rates
were measured on the running PS2.

**Method.** Each function was read in the decomp and, where Ghidra hides a detail (thunks with no header, VU0 field
masks, vtable slots), in the ELF. Three live PCSX2 runs (software renderer; the player spawned and his health
pinned) sampled every building and every soldier at 10 Hz: BF2 Mos Eisley and Yavin 4 (Clone Wars) and BF1 Naboo
Plains. One more run held an engineer player beside an unbuilt pad for the hologram's frame. An opus verifier got
the 37 claims with citations only: 33 supported, 4 partly (two counts, one citation range, one over-general "team
0"), none refuted; its corrections are folded in (section 8).

Tags: [V] read in the decomp / ELF at the cited lines, [L] measured on the live PS2 or in save-state RAM, [D]
derived, [H] hypothesis. B1 = `SWBFRONT.ELF-decomp.c`, B2 = `SWBFRNT2.ELF-decomp.c`. E = an entity, wc = a weapon
class.

## 0. Short answers

1. **A build is a repair to full health** [O][V][L]. A building has a build state: 0 built, 1 destroyed, 2 unbuilt.
   Its health-change handler calls "built" when health reaches MaxHealth while the state is not 0, and "death" when
   health reaches 0 while the state is 0. The built function's other callers are derived classes' wrappers of that
   same slot and a network-state reader (`FUN_0040dc88`) [V callers]: offline nothing else builds a pad [D].
2. **The tool is the fusion cutter** (weapon class `repair`) [O][V][L]. One shot adds a fixed amount by target
   type, one shot per `ShotDelay`. BF2: an unbuilt pad +50 every 0.5 s (a turret of 800 takes 16 shots, 7.5 s from
   the first to the last); a destroyed building +50; a damaged one +100; a vehicle +200. BF1: +30 / +20 / +60 / +60
   every 0.5 s (a gun turret of 300 takes 10 shots, 4.5 s); four BF1 sides carry a half-size cutter at twice the
   rate. Measured: 120 of 120 steps of exactly 50.0 on Mos Eisley, 0.5 s apart, five builds of 7.5 s; +30.0 on
   Naboo, ten builds of 4.40-4.51 s.
3. **A fallen turret regenerates by itself**: at death the building's regeneration becomes MaxHealth /
   `RespawnTime` a second, and full health rebuilds it [O][V][L]. An armed building's default `RespawnTime` is 120 s
   (BF1's gun turret: 2.5 HP a second, measured). BF2's TUR.LVL turrets say 999999.9, so they wait for a builder.
4. **Who builds** [O][V][L]: a bot whose class `UnitType` is repair / support / special or pilot, that carries a
   cutter with a positive amount for the pad's type. Every 5 s it looks for a friendly or neutral thing within 75 m
   that lacks health; an unbuilt or destroyed building qualifies though it is "dead". One bot per team may claim a
   building. It walks to 1.6 m from the pad and holds the trigger. BF2: engineers (`UnitType` pilot) and Mos
   Eisley's Jawas (support). BF1: the pilots. Measured: all 120 build steps on Mos Eisley came from Jawas, all 48
   on Yavin from CIS engineers, 183 of 184 on Naboo from the two sides' pilots.
5. **The hologram is drawn only for a player who carries a cutter** [O][V][L]: a hologram of type `build` is hidden
   per screen unless that screen's soldier is alive and holds a `WeaponRepair` in any slot. It is on from the
   building's construction while unbuilt, off when built, back on when the turret falls to its pad. The frame
   (`lead_tmp/tbld/ps2/yav1_holo_1b8b5c0_1.png`): a green icon turning over the grey pad.
6. **Two PS2 quirks the port should not copy** [L][V]: (a) BF2 bots that reach the pad's centre (under 1 m) stand
   there and build nothing until a 20 s timeout - a think-rate artifact: 22 of the 79 serve spells at pads on Mos
   Eisley ended that way, and only 17 at a built pad; BF1's rule has no such dead zone. (b) A BF2 bot that has just
   finished one pad keeps the trigger held and builds its next pad from up to 59 m away while walking to it (28 of
   120 steps): the AI hands the weapon its target with no range test.

## 1. The building's build state [O]

| | BF2 | BF1 |
|---|---|---|
| state word | E+0x274 | E+0x2a4 |
| health / max / regeneration | E+0x170 / +0x174 / +0x178 | E+8 / +0xc / +0x10 |
| flags: bit 3 alive, low three bits the repair target type | E+0x228 | E+0xac |
| hologram entity | E+0x280 | E+0x2b0 |
| class: unbuilt model / destroyed model / RespawnTime | +0x4d4 / +0x384 / +0x640 | +0x156c / +0x14b4 / +0x162c |
| handler / death / built | FUN_001e3458 / FUN_001e3638 / FUN_001e3c38 | FUN_001bfa88 / FUN_001bfb88 / FUN_001c00c8 |

- [V] **Every health write goes through the handler.** `FUN_001df3d8` stores the new health and calls slot +0x28 of
  the damageable's vtable (pointer at damageable+0xbc = E+0x22c) with the difference (B2:90997-91002). The building's
  slot +0x28 is the thunk 0x1e5378 -> `FUN_001e3458` (ELF `jal 0x1e3458`, `a0 - 0x170`; no decomp header).
- [V] **The handler** (B2:93483-93492): health <= 0 -> health = 0 and, if the state is 0, slot +0xc (death); else
  health >= max -> health = max and, if the state is not 0, slot +0x10 (built). The built call is skipped while a
  network-client expression holds; its four flags are all 0 in the ELF and in seven offline battle states, and the
  only script that sets them is the multiplayer host screen [L, verifier]. BF1: B1:66651-66660.
  An armed building swaps the vtable for 0x61a368 (B2:96404): slot +0xc `FUN_001e8498` and +0x10 `FUN_001e85a8`
  switch its turret sections off / on and then call the same death / built functions (B2:96922, 97015).
- [V] **Death** `FUN_001e3638` (B2:93584-93789): nothing when the state is already 1 or 2; clears the alive bit.
  No destroyed model and an unbuilt model -> **state 2**, target type 6, the unbuilt model and its collision, the
  hologram on. No destroyed model, no unbuilt model -> state 1, type 5, hidden. A destroyed model -> state 1, type
  5, hologram off. Then **regeneration = MaxHealth / RespawnTime** (93789). The type bits are written when the
  class's HealthType field (class+0x2e4) is 4, its default. BF1: B1:66696-66852.
- [V] **Built** `FUN_001e3c38` (B2:93830-93990): state 0, alive bit set, target type 4, the class's model and
  collision back, hologram off, regeneration = the class's AddHealth (class+0x2f0), health = max. BF1: B1:66888-67019.
- [V] **Construction** `FUN_001e2d40` (B2:93220-93361): no unbuilt model -> state 0. Else state 2, type 6, alive bit
  clear, health 0, regeneration 0, and the class's `UnbuiltHoloOdf` (class+0x624) is instanced at the unbuilt
  model's node 0xd1e4cc7a (else at the building) and kept at E+0x280. BF1: B1:66399-66405.
- [V] **Regeneration runs in the building's update**: `FUN_001df560` adds dt x regeneration while health < max,
  through the same setter (B2:91047-91052; called from `FUN_001e8130` B2:96717 and `FUN_001e6ec0` B2:95822).
- [V] **RespawnTime** (key 0xbd94b44e) -> class+0x640 (B2:94488). The base building class defaults to FLT_MAX
  (B2:94210); the **armed building's class sets 120.0** (`FUN_001e8860`, B2:97087; its create slot builds the entity
  0x61a470, B2:97223). BF1: FLT_MAX at B1:67217, 120.0 in `FUN_001c4cd0` (B1:70305, RTTI `19EntityBuildingArmed`).
- [L] RAM: an initially unbuilt pad has health 0.00 and regeneration 0; a BF2 turret that fell has state 2, health
  0.00, regeneration 0.0008 (= 800 / 999999.9), flags 0x26; a fallen `tur_bldg_tower` (destroyed model, no unbuilt
  one) state 1, flags 0x25. BF1 Naboo: a fallen gun turret carries regeneration 2.5 and rises 2.50 HP a second
  (8,366 samples). A pad is team 0; a built, manned turret carries its gunner's team.
- [L] Half-built pads keep their health (400 and 600 held for 100 s and more on Mos Eisley). Whether a pad can be
  damaged was not tested [H: no - its alive bit is clear].

Disc (`lead_tmp/tbld/tools/cls.py`): buildable classes are BF2 `tur_bldg_laser` / `_beam` / `_chaingun` /
`_guided_rocket` (MaxHealth 800, RespawnTime 999999.9, `com_holo_turret`), two `com_item_*recharge_unbuilt`
power-up stations (one instance, GEO1), and campaign props; BF1 `com_weap_gunturret` (MaxHealth 300, no
RespawnTime key -> 120 s; 129 instances on 12 maps: BES2 5, END1 3, KAM1 16, KAS2 16, NAB1 21, NAB2 9, RHN1 14,
RHN2 10, TAT1 12, TAT2 11, YAV1 6, YAV2 6). Other RespawnTime values: GEO's geoturret 30 (both games), BF1
`yav_bldg_tower_turret` 50, BF2 `uta1_bldg_turret` 99999.

## 2. The fusion cutter [O]

- [V] **Amounts by target type.** The class setter (B2 `FUN_00347d48`, B2:340311-340459; BF1 `FUN_00292f60`,
  B1:218733-218897) fills eight floats at wc+0x244 (BF1 seven at wc+0x1e4), indexed by the target's type = the low
  three bits of its flags word:

  | type | 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 (BF2) |
  |---|---|---|---|---|---|---|---|---|
  | key | PersonHealth | AnimalHealth | DroidHealth | VehicleHealth | BuildingHealth | BuildingRebuild | BuildingBuild | MineHealth |
  | BF2 `com_weap_inf_fusioncutter` | - | - | 0 | 200 | 100 | 50 | 50 | -1000 |
  | BF1 all / cis / imp / rep | - | - | 0 | 60 | 60 | 20 | 30 | |
  | BF1 gar / gun / wok / jawa | - | - | 30 | 30 | 30 | 10 | 15 | |

  A second table at wc+0x264 (BF1 wc+0x200) holds the ammo twins (`VehicleAmmo` ...; no cutter sets one).
  `ShotDelay` is copied to wc+0x240 (default 0.125): 0.5 on every cutter but BF1's four half-size ones (0.25, so
  the same 60 HP a second on a pad). BF2's seven side cutters inherit the base and override no amount. A non-zero
  amount also sets the type's bit in the class's target masks (wc+0x120 / +0x124).
- [V] **One shot.** `FUN_00346fa0` (B2:339845-339910): target = the weapon's handle (+0x10c, generation +0x110),
  else the owner itself; amount = scale x wc[0x244 + 4 x type]; added by `FUN_00369770` -> `FUN_003696e8`
  (B2:364176-364199): only when health < max and regeneration >= 0 (or the amount is negative), new health =
  min(health + amount, max) through the setter of section 1. **No distance is tested in the shot** (whole function
  and disassembly scanned by the verifier). BF1: `FUN_00292ce0` (B1:218640-218674) -> `FUN_002ab428`.
- [V] **The cycle.** `FUN_00346c78` (B2:339681-339721): with a positive delay the first shot lands at once (scale
  1.0), then a timer runs to the delay; while the trigger is held and the target still takes health, the flag
  clears and the next shot lands. With a delay <= 0 it would act every frame scaled by the frame time. BF1:
  `FUN_002929b8` (B1:218493-218532), the delay loaded by `FUN_00292980`.
- [V] **Whose target.** A player's weapon gets its handle from the acquisition (`re-aim-assist.md` 3: inside the
  cone `LockOnAngle` and `LockOnRange` + the target's radius, the class's team and type masks): BF2 4.0 m and 90
  deg, BF1 3.0 m and 30 deg (6.0 m for the half-size cutters). An AI's weapon gets it from `FUN_002bbb98`
  (B2:242516-242531): the same two masks, **no range** - quirk 6 (b).
- [D] Heat: `HeatPerShot` 0.06, `HeatRecoverRate` 0.16, `HeatThreshold` 0.7 drain an energy bar (`FUN_001e2010`).
  16 shots cost 0.96, so one bar builds one BF2 turret; whether the bar recovers while firing was not traced [H].
- BF2's cutter also has `TargetEnemy 1` (it "slices" an enemy vehicle: a branch of `FUN_00346fa0` that counts its
  occupants and adds the shot delay to a meter) and `MineHealth -1000`. Not read further: outside this card.

## 3. The bots that build [O]

BF2: state 6 of the trooper agent (`re-soldier-ai.md` 4.8, where the search was first read). BF1: its own agent.

- [V] **Who.** BF2: the class's `UnitType` number is 2 (repair / support / special) or 3 (pilot) (B2:261420). BF1:
  `FUN_002108a0` (B1:123637-123668) maps scout 0, assault 2, repair / support / special 3, pilot 4, trooper 5; the
  agent factory gives kinds 3 and 4 the **UnitRepairAgent** (`FUN_00249528`, vtable 0x536d88; B1:182986-183001).
  Disc: BF2's engineer classes say `UnitType pilot` and carry the cutter in slot 2 (channel 0); `tat_inf_jawa` says
  support; `wok_inf_mechanic`, `gun_inf_rider` and `gun_inf_soldier` carry a cutter with no `UnitType` in their
  class chain [D: the default kind 5 fails the test], so their bots never build. BF1's pilots say pilot, cutter in
  slot 3 (channel 1).
- [V] **The search**, every 5.0 s (BF2 `FUN_002d6828`, B2:261419-261558; BF1 `FUN_00249908` -> `FUN_00249f68`,
  B1:164370-164812): up to 50 friendly or neutral entities within 75 m. A candidate must be alive **or of type 5 or
  6** (B2:261452-261453, B1:164727-164728), have a free claim, lack health its type's amount can give (and no
  power-up of that kind within 15 m), and - unless it carries a human within 30 m - be at rest (speed under 0.5 m/s)
  and not an AI soldier; the nearest wins. BF2 also skips a class flag (class+0x334 bit 0) and an owner-team test.
  BF1 does no search for a team under its low-ticket gate (`FUN_00236e20`).
- [V] **One builder per team per building**: the claim count of type 0 is capped by the entity's slot +0xa4, which
  for the armed building returns 1 (ELF 0x26bb30 `li v0,1`); a claim row lives 10 s and is renewed on every think
  of the serve state.
- [V] **The serve state** (BF2 `FUN_002d76a8`, B2:262009-262097): the bot is in position when its horizontal
  distance d to the target has d^2 < R^2 (the weapon's range) and (the weapon's target is X or d^2 < 25) **and, in
  BF2 only, d^2 >= 1**. Out of position it approaches: with a clear line `FUN_002bc9a0(1.6, ..)` walks at the
  entity and stops 1.6 m out; without one it plans a route to the entity's position, at most every 8 s
  (`FUN_002a7d40`, B2:228440-228483). After 20 s out of position with an idle or failed mover it gives up. In
  position it stops, stamps the time and holds the trigger when its aim is on X (`FUN_002d7510`); every 8 s it gives
  up if X's health has not risen. BF1 `FUN_00256fe8` (B1:174298-174345): the same constants, **no d^2 < 1 term**.
- [L] Mos Eisley (240 s, about 40 soldiers, the player an idle engineer): 79 serve spells at pads; 17 ended at a
  built pad, 3 by death, 4 with the capture, 55 were left - 50 of those with no health gained, 22 with the bot
  standing under 1 m from the pad's centre (the route ends on the centre and no think fell inside the 1-5 m ring).
  A building bot stands 1.50 m or a little more from the centre (median 1.57). The Republic and CIS engineers
  kept breaking off into fights (the agent changed to the combat agent); the Jawas, neutral to both, built.

## 4. The hologram [O]

- [V] The class (`hologram`): `HoloType` "aligned" -> 1 (coloured by the viewer's relation, the command post's) and
  "build" -> 2 (B2:130355-130365); `HoloVisibleDistance` -> class+0x68. `com_holo_turret`: colour 0 225 98, type
  build, image `com_icon_turret`, visible distance 35, height 0.75, size 1, beam 0.3, image 0.5, rotate 0.2 turns a
  second, flicker alpha 0.4-0.8, fade-in 2 s.
- [V] **Per screen**: the update of a type-2 hologram sets its hidden bit for screen v unless that screen's flag
  word has bit 0 (B2:129486-129503; BF1 B1:94806-94826); the render does nothing for a hidden screen (B2:129895)
  and draws the image only inside the visible distance, fading from 0.9 of it (B2:129959-129963).
- [V] **The flag** comes from the screen's tracked object (B2 `FUN_001bbfb8`, slot +0x1c; BF1 `FUN_001aa928`, slot
  +0x18). The soldier's version (B2 `FUN_0024daa0`, B2:167276-167288; BF1 `FUN_0020c5b8`, B1:121120-121132) sets
  bit 0 when the soldier is alive and any of its weapons is a `WeaponRepair`. Every other vtable of that family
  holds an empty function in the slot (14 of 15 in BF2, 11 of 12 in BF1; verifier), so a player seated in a
  vehicle or a turret sees none [D].
- [V] On / off: on at construction (B2:129307-129310), off when built, on again at a fall to the pad (section 1).
- [L] An engineer player alive: flag word 0x1, hidden mask 0 on all 11,755 unbuilt-pad samples of Mos Eisley; every
  stored state with another class, or no soldier, has mask 1 (26 of 26 pads); the mask went to 1 the moment the
  held player died. Frame: `lead_tmp/tbld/ps2/yav1_holo_1b8b5c0_1.png` (crop `yav1_holo_crop.png`).

## 5. Measured on the PS2 [L] (`lead_tmp/tbld/ps2/`, tools in `lead_tmp/tbld/tools/`)

| | BF2 Mos Eisley CW | BF2 Yavin 4 CW | BF1 Naboo Plains |
|---|---|---|---|
| build steps | 120 x 50.0 | 45 x 50.0, 3 x 49.94-49.95 | 120 x 30.0, 58 x 30.2-30.3 (+ regeneration) |
| gap between steps | 0.5 s (110 of 112) | 0.5 s (34 of 40; 0.4 / 0.6 the rest) | 0.5 s (99), 0.4 (34), 0.6 (25) |
| a build from 0 | 16 steps, 7.5 s first to last (five builds) | - | 10 steps, 4.40-4.51 s (ten builds) |
| builders | `tat_inf_jawa`, 120 | `cis_inf_engineer`, 48 | the two sides' pilots, 183 of 184 |
| builder distance | 1.50-4 m 92, 5-59 m 28 | 1.42-4 m 40, 10-31 m 8 | 1.06-4.54 m |
| built over time | 3 of 11 at 55 s, 5 at 130, 6 at 160, 7 at 205, 8 at 235, 9 at 250, 10 at 265 | 3 of 6 pads built at the start; two fell and were rebuilt 65-98 s later; 3 far pads never | 7 of 21 at 0, 12 at 45, 8 at 75, 13 at 120, 17 at 165, 20 at 225 |

The times are the mission clock of a battle already running (the kept side-select states); the 10 Hz sampling puts
+-0.1 s on a gap. BF1's turrets are built, shot down (300 HP) and rebuilt all battle.

## 6. For the port

- The sim needs the building's three states on its turrets: unbuilt at the start for a class with an unbuilt model
  (BF2 now; BF1 with card BZONE1), a fall to the pad or to "destroyed" instead of removal, regeneration MaxHealth /
  RespawnTime (120 s default for an armed building) and "built at full health" (card TBLD's build).
- The cutter: a `repair` weapon class with the amount table, one shot per ShotDelay while the trigger is held and
  the acquisition holds a target the masks allow; the hologram for a living cutter carrier (card FCUT).
- The bots: the 5 s / 75 m search for building targets, one claim per team, the 1.6 m approach, the 20 s and 8 s
  give-ups (card BLDR). **Named deviations**: BF1's position rule on both games (no dead zone under 1 m), and no
  building from afar - the port's tool needs its target in reach. Both make the port's bots build faster than
  BF2's; the PS2's own curve is section 5.
- Not ported here: mines, the ammo table, a bot serving humans (card ENGR). Slicing: section 11.

## 7. Still [H] / not read

- A pad's damage while unbuilt; the energy bar's recovery while firing; why a far-building bot's trigger lets go
  (it stopped at 7-8 m in one episode, as the "within radius + 8 m" branch would have it).
- The class flag behind key 0x43de19e2 (class+0x655) and the object at E+0x284 it toggles on built / death (an AI
  barrier switch by its shape).
- BF2's second FSM that calls the same search (`FUN_002d31a8`); BF1's pilot-in-turret regeneration (built turrets
  with regeneration 1.5 on Naboo, 1,390 samples) - one for card ENGR.
- The node 0xd1e4cc7a's name; the built sound's key 0x66aab312 is `BuiltSound` (no turret class sets it).

## 8. Checks run

- `re_check.py fn` on every cited line, `word` on the vtables 0x619a40, 0x61a2e0, 0x61a514, 0x61e948, 0x52ee30,
  0x532b80, 0x536d88, `callers` and `absent` on the type ids: `lead_tmp/tbld/re_checks.txt`.
- Verifier (opus, foreground, claims only; `lead_tmp/tbld/verify/claims.md`, `VERDICTS.md`): 33 SUPPORTED, 4
  PARTLY, 0 NOT SUPPORTED. Corrected from it: BF1 has two cutter value sets; seven BF2 side cutters inherit, not
  eight; five 7.5 s builds, not four; a built, manned turret carries its gunner's team (only pads are all team 0);
  `FUN_00292f60` ends at B1:218897. Added from it: the armed building's own vtable 0x61a368 and its wrappers; the
  network flags' defaults and writers; the 10 s life of a claim row; that only the soldier's vtable fills the flag
  slot; the far rises are three episodes in which the mover's aim order was not the pad.
- Reproduced by me after the verifier: the BF1 half-size cutters (`cls.py bf1 dump gar_weap_inf_fusioncutter ...`),
  the four non-zero team rows (`pads2.py`), B2:96404; added after it: the built function's other callers
  (`re_check.py callers b2 FUN_001e3c38`: `FUN_001e7160`, `FUN_001e85a8`, `FUN_003512a8`, `FUN_0040dc88`), the
  serve-spell endings and BF1's ten unbroken builds.
- Hashes cracked here (FNV over bytes | 0x20): 0xf5ed2064 `MineHealth`, 0x8f2a8613 `TargetMine`, 0x059df9f0
  `ScoreForMedalsType`; the hologram's keys by the mod tools' ODF guide (`COLOR`, `HoloType`, `HoloImageGeometry`,
  `HoloVisibleDistance`, ...).

## 9. Tools and data (`engine/artifacts/lead_tmp/tbld/`)

- `tools/cls.py bf1|bf2 dump|base|find|prop` - every entc / wpnc / ordc / expc class on a disc, keys named from
  the mod tools' guides and the PC executable's strings.
- `tools/tbld_bf2.py <side-select state> <mission> --stem S --rights 3` - the live sampler (player spawned as the
  class `--rights` presses right of the first; pads, soldiers, serve helpers, cutters, claims at 10 Hz);
  `tools/tbld_bf1.py <spawn state> --stem S` - BF1's. `tools/holo_shot.py` - holds the player by a pad and grabs.
- `tools/an_pads.py`, `an_fire.py`, `an_units.py`, `an_bf1.py` - steps, gaps, builds, the firing builder, serve
  spells; `tools/pads2.py <states>` - buildings and nearby soldiers of a stored BF2 state.
- `ps2/tat2_a_*`, `yav1_a_*`, `nab1_a_*` (CSVs, end states), `ps2/yav1_holo_*` (states + frames).

## 10. Service-droid cutter support in the port (2026-10-26, DROIDR)

The runtime loads `repair` weapon classes without requiring ordnance. Their eight health amounts come from the
class chain, including `BuildingHealth`, `BuildingRebuild` and `BuildingBuild`. BF1's channel-1 pilot cutter
and BF2's channel-0 engineer cutter use the existing weapon selection inputs. The side-asset bake includes the
four factions' cutter hand models and textures from each disc. Both channels use one fixed-step cutter path in
`sim/repair.rs`: it acts immediately and then at the authored `ShotDelay`, with no bolt, grenade or simulation
RNG draw. Repair pulses consume `HeatPerShot` of the soldier's energy bar. Acquisition shares the existing
cone scoring and checks relation masks, range and terrain/world occlusion, skipping the target's own body.

An intact damaged service droid receives its health-type repair amount; a destroyed Building station receives
`BuildingRebuild`. The station owns this repair-type selection. Partial progress retains the wreck and does
not serve anyone. At MaxHealth the station becomes intact, restores its live collision and aim sphere, rebinds
projectile hits, and resumes its existing clock and animation. The viewer and ambient paths follow its
restored state. Initial loading and destruction share collision caching in `sim/stations.rs`. Collision
replacement reuses body slots across repeated rebuilding. Repair events carry the amount and target, feed the
existing FireSound key and appear in headless event CSVs. Combat weapon selection excludes repair tools.

This covers service-droid repair/rebuilding and basic friendly/neutral vehicle healing through the same
cutter. Autonomous builder orders, turret pads, initial unbuilt-station placement, holograms, enemy vehicle
slicing and mine removal remain separate work described above. BuildingBuild is retained as class data;
service-droid wrecks use BuildingRebuild. No new PS2 observations or pixel/audio parity claims are made.

Regressions cover both games' cutter channels/rates, range/cone/team/occlusion/energy gates, partial progress,
restored collision and sphere, repeated rebuilding, service resumption and deterministic hashes. Data-backed
cases load original cutter tables and medical/ammo droids on BF1 Bespin Platforms and BF2 Hoth. The channel
tests use the spawn-selected weapons directly, without overriding their state.

Validation: workspace formatting, Clippy with warnings denied, build and all unit/integration/arena/doc test
targets passed for the implementation. The stale manifest field-order fixture was updated to include the
already-serialized water `wet` field. Cleanup was checked with all 999 core tests, 413 game library tests (two
existing ignored), and the station integration tests; the preceding full game suite passed 220 tests with one
existing ignored. Logs live under `engine/artifacts/lead_tmp/pickups-rebuild-*.log`. The local package
includes eight cutter models and passed archive/prefix and 100/600-tick smoke checks. No interactive playtest
or full-round sweep is claimed.

## 11. Slicing an enemy vehicle (OPUS RE 2026-10-29, issue #35) - [O] UNAUDITED: one reader, no verifier

**Answer.** BF2's cutter on an enemy vehicle with soldiers aboard fills a slice meter by the cutter's `ShotDelay`
per shot; when the meter passes the class's `TimeRequiredToEject` every soldier aboard is put out, the meter goes to
0 and the vehicle takes a `TimeTilReboard` lock (5 s) during which CanEnter refuses everyone. Nothing in the branch
changes the vehicle's team. BF1's cutter has no such branch.

- [V] **The branch** (B2 `FUN_00346fa0`, B2:339816-340204; the earlier citation 339845-339910 covers only its
  start). After the target's IsA tests route a vehicle to *(decompiler detail omitted)* (B2:339995), `FUN_002693f0(owner, target)`
  (B2:186036: `FUN_002692b0(...) < 0`, the relation is enemy) must hold, the target's slot +0xe0 must not answer 5,
  and the target must not be IsA `DAT_0069a420` / `DAT_0069a570`; otherwise the shot is an ordinary repair
  (*(decompiler detail omitted)*, `FUN_00369770`) - so an EMPTY enemy vehicle is healed.
- [V] **The count** (B2:340002-340040): every seat whose occupant is IsA `DAT_00689828` (the soldier) counts. With
  none, or with the class's slot +0x48 (the threshold) <= 0, the shot is an ordinary repair.
- [V] **The meter** (B2:340045-340050): meter (slot +0xec) := meter + wc+0x240 (`ShotDelay`, 0.5 s on every BF2
  cutter) through slot +0xf0. When threshold < meter: a script event fires (`DAT_00720f1c` list), then for every
  soldier aboard (B2:340100-340180) its controller's slot +0x78 and slot +0x70(.., 0, 1) run (the exit), then the
  meter := 0 (B2:340185) and E+0x188 := the class's slot +0x4c (B2:340188), and the cutter's sound (wc+0x1e0)
  plays. With 0.5 s steps the eject lands on the shot that takes the meter past the threshold: a 10 s class takes 21
  shots, 10.0 s from the first to the last [D].
- [V] **The class fields.** Slot +0x48 / +0x4c of the class tables, read in the ELF (`eedis.py bf2`): hover class
  (vtable 0x61c9d8, ctor `FUN_00221a20`) `lwc1 f0, 0xf7c(a0)` / `0xf84(a0)`; walker (0x61f5b8, `FUN_00263160`)
  +0xfc0; flyer (0x61bff0, `FUN_0020f098`) +0x11fc; every other entity class table holds `0x0026bcd8` (`mtc1 zero`,
  never sliced) in the slot. Ctor defaults: hover 3.0 / 5.0 (B2:136736-136738), walker 12.0 (*(decompiler detail omitted)*,
  B2:182455), flyer 4.0 (B2:124430). Setter keys (B2:137205, 137922): hash 0xbb9297d5 = **`TimeRequiredToEject`**,
  0x165e67e0 = **`TimeTilReboard`** (cracked here from the ELF's own strings, FNV over bytes | 0x20).
- [V] **Disc values** (`engine/assets_baked/classes/bf2_*.ron`): AAT 10.0, snail tank 10.0, hailfire 5.0, STAP
  1.0, BARC / speeder bike 1.0, fighter tanks and combat speeder 7.5, AT-ST 10.0, one-man AT-ST 5.0, spider
  walkers 12.5, starfighters 2.5; `all_hover_bos_combatspeeder` -1 (never sliced); `TimeTilReboard` 5.0 on all.
  `commandwalker` / `commandhover` bases (AT-AT, AT-TE, MTT) are other entity classes [H: their tables hold the
  zero getter; not read word by word].
- [V] **The lock.** E+0x188 > 0 makes the two CanEnter tests return 0 at once, with no test of who asks (B2
  `FUN_0024a9d0` B2:164843, `FUN_0024ac98` B2:164976).
- [V] **The occupants' alert.** The vehicle HUD (`FUN_003b4bb8` -> `FUN_003b4a08`, B2:410618-410670) shows
  Locl 0x67fdfe71 "! Slicer Alert %d%% !" (meter / threshold x 100) while the meter is over 0.1.
- [V] **BF1**: `FUN_00292ce0` (B1:218640-218674) is the plain repair; no occupant count, no meter.
- [V] **Bots**: the builder's search takes friendly or neutral things only (section 3), so the PS2's bots never go
  to slice; only a player's acquisition hands the cutter an enemy vehicle.
- [H] Whether the meter decays between shots (slots +0xec / +0xf4 not opened).

**Port (2026-10-29, the maintainer's rules).** Players slice on both games (BF2's rule on BF1 too, named deviation;
BF1 classes take the PS2 class-table default for their kind: hover 3 s, walker 12 s, flyer 4 s). Command walkers and
command hovers are never sliced. After a slice only the slicer may board for 3 s (ours, `SLICE_RESERVE`) instead of
the PS2's 5 s lock on everyone. Bots do not slice. Ejected riders leave through the ordinary exit.

## 12. Initial BF2 pads and the build hologram (2026-11-01, issue #79)

BF2 armed buildings with `UnbuiltGeometryName` now register as repairable pads
at zero health. Previously the scene drew their pads but the runtime skipped
these turret definitions, so neither a cutter nor an engineer could build them.
The intact class geometry, collision and posed gun rig remain available; initial
construction removes the empty vehicle without a death effect, and full-health
repair restores a fresh vehicle and the intact body bindings. No regeneration
is added. BF1's initial spawning remains unchanged; its fallen pads can use the
same build hologram.

The existing engineer objective handles these initial pads as it already handles
fallen turrets. Real PS2 Yavin and Naboo data, in Clone Wars and Galactic Civil
War, exercise the 800-health pool and sixteen +50 cutter pulses at 0.5 s spacing
(225 simulation ticks, 7.5 s, between first and last). The one-builder claim
policy remains the existing port policy, including deterministic resolution of
simultaneously chosen claims on the next think.

The view draws the original `com_icon_turret` over a green beam, using
`com_holo_turret`'s 0.75 m height, size 1, 0.3 beam intensity, 0.5 icon intensity,
35 m visibility, 0.2 turns/s, 2 s fade and 0.4–0.8 flicker. A separate cosmetic
random stream preserves the command posts' flicker. The model comes from the map
bake and its sheet can come from the common bake. Per screen, only a living,
unmounted soldier carrying a repair weapon in either channel sees build
holograms; partial health keeps them on, full health hides them, and a fall to
an unbuilt pad enables them again. A destroyed-model wreck has none.

The build hologram uses the building origin; the unnamed node mentioned in
section 1 is still untraced. The shared hologram implementation supplies its
existing beam-base width and pop behavior. These scoped headless tests do not
claim a fresh original-game paired capture or acceptance of every map.
