# OPUS RE (2026-09-30) - [O] UNAUDITED
# RE: the muzzle flash — a tinted additive model at the weapon's fire point, one per shot (card FXMF)

Verified: one foreground Opus verifier, 20 claims: 18 SUPPORTED / 2 PARTLY (both outside the flash's own path, folded in)
/ 0 NOT SUPPORTED (§10).

Card **FXMF** (M1, alpha), loop lane 2, effort max. Scratch (gitignored): `engine/artifacts/lead_tmp/fxmf/` — `census.py`
→ `census_bf1.txt` / `census_bf2.txt` (every disc weapon class's flash after the setter rules), `skeldump.py` (weapon
skeletons), `disctex.py` (disc textures), `ramflash.py` / `ramflash2.py` / `ramflash_bf2.py` / `playerweap.py` (the flash
fields and timers in PS2 RAM), `probe_bf1.py` / `probe2.py` (PCSX2: natural fire + RAM knock-ins), `sheet.py` / `sheet2.py`,
`ps2cap/` (states, sheets), `offscan.py` / `cobase.py` / `cooc.py` (field scans), `checks.sh` → `checks.txt`,
`verify/CLAIMS.md` + `VERDICTS.md`. B1 = `analysis/SWBFRONT.ELF-decomp.c`, B2 = `analysis/SWBFRNT2.ELF-decomp.c`.
Tags: **[V]** read at the cited lines (and re-read by the verifier); **[D]** derived; **[H]** hypothesis (PROVISIONAL in
code); **[O]** load-bearing for the build; "RAM" = read in PS2 save states; "asm" = checked in the instruction bytes
(`scripts/tools/eedis.py`); "PCSX2" = seen on the PS2 itself.

## 0. TL;DR — what the port needs

- **It is a model, not a sprite** [V][O] (§4). Each shot restarts a timer on the weapon; while it runs, the weapon's render
  draws the model `muzzleflash` (BF1 COMMON) / `com_sfx_muzzleflash` (BF2 INGAME) — 4 crossed planes along +Z, 1.056 m long
  and 0.54 m wide, plus a small cone at the base — at the weapon's **fire point** (the `FirePointName` node, default
  `hp_fire`, of the `GeometryName` model), its +Z along the soldier's **aim direction**, scaled uniformly by
  **FlashRadius**, tinted by **FlashColor**, drawn **ADDITIVE** (GS 0x48), depth tested, **no depth write**, unlit.
  The texture is white; its **alpha is the flash's shape** (a soft teardrop flame and a round glow).
- **Timing** [V][O] (§3). Shot → end = now + **FlashLength** (seconds, game clock). Each render: t = (end − now) /
  FlashLength; drawn while t > 0 with alpha byte = (int)min(A, 2·A·t) (A = FlashColor's alpha): full for the first half,
  then a linear fade. **Rifles and pistols author FlashLength 0.025 s — less than one 30 fps frame — so on the PS2 each
  shot's flash is drawn in exactly the frame of the shot, at full alpha** (RAM: t = +1.000 in that frame, −0.33 in the next).
- **The presets** [V][O] (§1). `MuzzleFlash = small_ / med_ / large_muzzle_flash` sets FlashRadius 0.6 / 1.0 / 1.8,
  FlashLength 0.1 / 0.15 / 0.3 and FlashColor (255, 255, 50, 255); any other value does nothing. Keys after it override
  (every infantry blaster sets FlashColor and FlashLength = 0.025 after `small_muzzle_flash`). The label default is
  FlashRadius −1 = no flash.
- **Unread** [V][O] (§6): **FlashWidth** and **FlashLightColor / FlashLightRadius / FlashLightDuration** — parsed, copied,
  never read by either ELF (instruction scans + the verifier's base tracing), and knocking them in on the PS2 changes
  nothing (no green light at 30 m, FlashWidth ×33 → the same flash). The muzzle flash makes **no light** in either game.
- **Who flashes** [V][O] (§5): soldiers' held guns (the soldier render draws the weapon, and the weapon's render draws
  the flash) and the first-person view — **not vehicle, turret or building guns** (nothing else calls the weapon render),
  **not sabres** (WeaponMelee's render override never reaches the draw), not thrown weapons (weapon+0xbc hides them).
- **The port today** (§8): a PROVISIONAL yellow quad at a guessed offset; the baked `meshmuzzleflash` texture lost its alpha
  (ALPHA RULE v2 rule 2 counts bit 0x80 as modulation → OPAQUE), and the baked weapon models carry no fire point.
  → **FXMF → build**: `core::flash` (the look, replayed like `LaserLook`), the fire points and the texture alpha in the bake,
  an additive tinted model draw at `hp_weapons` × fire point.

## 1. The keys, the fields, the defaults (both ELFs) [V][O]

Setter B1 `FUN_002864f0` (header 209793), B2 `FUN_00328fb0` (319917). Names hash as FNV-1a over byte|0x20 (all recomputed by
the verifier). Floats parse `%f`; colours `"%i %i %i %i"`, each missing value 0xff.

| key (hash) | B1 class off | B2 class off | setter rule (B1 lines) | label ctor default (B1 `FUN_00285a98`, B2 `FUN_00327e98`) |
|---|---|---|---|---|
| MuzzleFlash (0xcfcccbce) | — | — | preset, below (210365–210409) | — |
| FlashRadius (0x86fdc077) | +0x1a4 | +0x1fc | float (210227–210228) | **−1.0** (209492; asm `lui 0xbf80`) — no flash |
| FlashWidth (0x94650003) | +0x1a8 | +0x200 | float (210273–210274); **unread** | 0.25 (209493) |
| FlashLength (0xb7b2d011) | +0x194 | +0x1c8 | float, seconds (210337–210340) | 0.1 (209494) |
| FlashColor (0xeb4230ac) | +0x1ac..+0x1af | +0x204.. | bytes R, G, B, A (210451–210462) | (255, 255, 50, 255) (209510) |
| FlashLightColor (0xf2e54d72) | +0x1b0.. | +0x208.. | ≥ 4 values: (r·a/255, g·a/255, b·a/255, 0xff); 3: (r, g, b, 0xff) (210487–210506); **unread** | (127, 127, 25, 255) (209511) |
| FlashLightRadius (0x83eea041) | +0x1b4 | +0x20c | float (210245–210246); **unread** | 0 (209512) |
| FlashLightDuration (0xa3bfd787) | +0x1b8 | +0x210 | float (210263–210264); **unread** | 0 (209513) |
| MuzzleFlashModel (0x1f63537b) | +0x19c | +0x1f4 | the model named, table 0x69c678 / 0x76c310 (209962–209966) | 0 (209522) |
| MuzzleFlashEffect (0x39aaa863) | +0x1a0 | +0x1f8 | an `fx__` effect (210121–210125); the fusion cutters' `repairkit` — FXP3's | 0 |
| GeometryName / FirePointName | +0x4c / +0x30 | +0x74 / +0x30 | the fire point, §4.1 (210114–210171; B2 320028–320078) | 0 / (0, 0, 0) |

- **Presets** [V] (B1 210377–210408, asm 0x28784c–0x287a1c; B2 320841–320880): the value's hash must be one of
  `large_muzzle_flash` 0xae94ded9 / `med_muzzle_flash` 0x84674572 / `small_muzzle_flash` 0xb92cf7a1:

  | preset | FlashRadius | FlashWidth | FlashLength | FlashColor | FlashLightColor | light radius / duration |
  |---|---|---|---|---|---|---|
  | large | 1.8 | 0.45 | 0.3 | (255, 255, 50, 255) | (191, 191, 37, 255) | 0 / 0 |
  | med | 1.0 | 0.25 | 0.15 | (255, 255, 50, 255) | (127, 127, 25, 255) | 0 / 0 |
  | small | 0.6 | 0.15 | 0.1 | (255, 255, 50, 255) | (63, 63, 13, 255) | 0 / 0 |

  Any other value returns without writing (BF2's `com_sfx_muzzleflash_lg` on `com_weap_veh_mortar` / `_fly_recoilless`,
  BF1's `lrg_weap_discharge` on the AAT launcher, `smgar_muzzle_flash` on the Gamorrean pistol — all no-ops).
- **Every disc class is a copy** [V] (C4). The class-type vtable's create slot (B1 0x53adec = `FUN_00287a40`) and the derived
  types' copy ctors all run `FUN_00285d70` (12 callers; B2 `FUN_003283d8`, 15), which copies the parent's flash fields
  (209639–209645; B2 319719–319726) but **sets the model afresh: `muzzleflash` (string 0x53ad90) / `com_sfx_muzzleflash`
  (hash 0x9db10c98)** (209648–209650; B2 319750–319751). So every weapon class on either disc carries the model; only a
  later `MuzzleFlashModel` PROP changes it (no disc class sets one). RAM: all 200+ weapon instances of BF1 NAB1 point at
  the one `muzzleflash` model (name hash at model+0x44 = 0xcfcccbce); BF2 HOT1 at `com_sfx_muzzleflash`.
- PROP order is the value (BF1 wpnc PROPs are the flattened ODF, parent first; BF2 keeps BASE = the parent class and
  inherits through the copy ctor). `census.py` replays both.

## 2. The census — what each disc weapon flashes [D] (`census_bf1.txt`, `census_bf2.txt`)

BF1: 151 wpnc classes, **68 carry a flash** (FlashRadius > 0, a model); BF2: 327, **182**. Only the soldier-held guns among
them ever show it (§5): by name, BF1 44 `_inf_` guns + 5 sabres + 19 vehicle/turret/building guns; BF2 78 + 24 + 80 (the 80
include held award and hero guns). The common rows (FlashRadius, FlashLength, FlashColor):

| game | R | L (s) | FlashColor | classes | what |
|---|---|---|---|---|---|
| BF1 | 0.6 | 0.025 | (255, 80, 80) | 20 | CIS/Imperial/Alliance blasters, pistols, snipers |
| BF1 | 1.0 | 0.15 | (255, 255, 50) | 18 | `med` preset: vehicle/turret guns and sabres (never drawn, §5) |
| BF1 | 0.6 | 0.1 | (255, 255, 50) | 16 | `small` preset untouched: launchers, remote droids, turrets |
| BF1 | 0.6 | 0.025 | (80, 80, 255) | 4 | Republic rifle/pistol/sniper/shotgun |
| BF2 | 1.0 | 0.15 | (255, 255, 50) | 52 | `med` |
| BF2 | 0.6 | 0.025 | (255, 80, 80) / (255, 100, 100) | 48 / 13 | blasters (`com_weap_inf_rifle`: 255 100 100) |
| BF2 | 0.6 | 0.1 | (255, 255, 50) | 29 | `small` |
| BF2 | 1.8 | 0.3 | (255, 255, 50) | 13 | `large`: speeder/turret cannons (never drawn, §5) |

Examples: BF1 `rep_weap_inf_rifle` R 0.6, L 0.025, (80, 80, 255); `all_weap_inf_rifle` (255, 100, 100); BF2
`rep_weap_inf_rifle` (100, 100, 255) over `com_weap_inf_rifle`'s (255, 100, 100). Every rifle also carries key 0x8b83f9b7
(`small_smoke_effect`, …; 157 BF1 / 506 BF2 PROPs) that **neither ELF reads** (no data word, no lui/ori, no text; C20).

## 3. The trigger and the clock [V][O]

- **Trigger** B1 `FUN_002859f8` (209364–209379): weapon+0xc0 = 1, **weapon+0xe0 = now + FlashLength**. It is slot +0x50 of
  every weapon vtable (0x53ae88 / 0x53b288 = 0x2859f8; the vtable sits at weapon+0x14). It is called when ordnance is born:
  the ordnance init `FUN_002728a8` calls it on the spawn packet's weapon (195216–195217; reached from the bolt/bullet spawn
  `FUN_002762a0` 197521), also `FUN_00278208` (198919–198920), `FUN_0028eb30` (215656), `FUN_00290b98` (217205). [D] So
  every round fired restarts the flash. B2 `FUN_00327d78` (slot +0x54, 0x627dfc): weapon+0xcc = now + FlashLength
  (319410–319414), called by the ordnance base ctor `FUN_0030eef8` (301502).
- The only other writer of the B1 end time is `FUN_00284148` (208268: now − 1.0, a reset); the instance ctor never
  initialises it (verifier C9).
- **now** = B1 `FUN_0033e4f0` = the game clock `0x56dcc8` in seconds (… B2
  `FUN_004240f8` (the clock 0x66ae90 via DAT_006fe398, ticks of 1/9000 s). It advances once per frame before the one
  `Update` (`analysis/a5-tickrate.md`), so the fire and the render of the same frame read the same now.
- **RAM** (`ramflash2.py`, `playerweap.py`, `sheet*.py`): in states saved while firing, the player's weapon reads
  t = +1.000 (end − now = exactly FlashLength) in the shot's frame and t = −0.335 (−1 frame) or −1.67 (−2 frames) otherwise —
  BF1 NAB1 mid1/mid2, KAS2 N0/N1/N4, BF2 HOT1 N0. The render stored its matrix in that same frame (§4.1), so [D] the
  0.025-s flash was drawn once, at t = 1, alpha = A.

## 4. The draw

### 4.1 Where [V][O]
The weapon's render (B1 `FUN_00285650` = WeaponCannon vtable 0x53b238 slot +0x64; B2 `FUN_003278a8`, slot +0x94) is handed
the weapon's world matrix M by the soldier render (§5). It draws the weapon model, copies M into weapon+0x20..+0x5c, and
replaces the translation with **P = M · (class+0x30)** (209197–209235; asm 0x285710–0x285744). class+0x30 is the
**fire point**: `GeometryName` loads the model and `FirePointName` (default `hp_fire`, CRC 0xb3ea0ee8) resolves the node
through `FUN_00496b70` (557539) — the node's origin **in its parent node's frame** (210114–210171; B2 `FUN_0057d858`,
320028–320078). With no model, or no such node, class+0x30 stays what the parent class had — (0, 0, 0) from the label.
- Disc census of every weapon skeleton (`skeldump.py`): 512 `hp_fire` nodes, all but one turret's (×6, parent
  `turret_main`) hang off an identity root, so the parent frame IS the model frame for every soldier weapon. E.g.
  `rep_weap_inf_rifle` (0, 0.0533, 1.097), `all_weap_inf_rifle` (0, 0.0597, 0.6926), `imp_weap_inf_rifle` (0, 0.0422,
  0.4082), `cis_weap_inf_pistol` (0.0103, 0.0394, 0.2601), `cis_weap_inf_sniperrifle` (0, 0.0188, 1.2953).
- **RAM** agrees class by class (`ramflash.py`: every class+0x30 above equals its skeleton's `hp_fire`); classes without a
  model — the super battle droid's wrist blaster/shrapnel/rocket, the remote droid, gun turrets — read (0, 0, 0): their
  flash sits at the weapon matrix's origin, i.e. **the `hp_weapons` node** (the wrist). `rep_weap_inf_launcher`'s skeleton
  has no `hp_fire` → (0, 0, 0) too (RAM and disc).

### 4.2 The matrix — orientation and size [V][O] (B1 `FUN_00287ba8` 210648–, B2 `FUN_0032b0e0` 321144–; asm, VU0 words decoded)
With R = FlashRadius, d = the aimer's direction (weapon+0x6c / B2 +0x70, then +0x50; **not normalised by the draw, unit
in RAM**: |d| = 1.00000 on every BF1 instance, 1.0000–1.0005 on BF2's) and ŷ = (0, 1, 0), the model is drawn with the row
vectors

    X = (d × ŷ)·R,   Y = (d × (d × ŷ))·R,   Z = d·R,   origin = P

— the model's +Z (the flame's length) runs along the aim; X and Y are **not normalised**, so the flash's cross-section
shrinks by cos(pitch) of the aim (a PS2 quirk: at 45° of pitch it is 0.71 as wide; straight up it would vanish).
d is the aim, not the barrel: in RAM the player's rifle axis and d differ by ~20° of pitch (BF1 NAB1 mid states).
The model then extends 1.056·R ahead of P and ±0.27·R across (R 0.6 → 0.63 m × ±0.16 m).

### 4.3 Colour, alpha, blend [V][O]
- Colour arg = (FlashColor R, G, B, (int)min(A, 2·A·t)) (asm `min.s`, `cvt.w.s` truncating).
- RedModel::Draw (B1 `FUN_00496248`, model vtable slot +0xc; flags 0): the colour is copied to a local (557216) and its
  alpha is faded near the far distance (557246–557256; the model far fade — irrelevant at muzzle range); nothing is drawn
  when the passed alpha is 0 (557270); the segment gets the local colour (557305–557311).
- The segment: BF1 `muzzleflash` MTRL **0x84** = Normal class, mode 2 (0x04 → translucent), +0x30 = bit 0x80 → **ALPHA
  0x48 = Cs·As + Cd (additive)**, TEST 0x51001 (Z test GEQUAL, **no Z write**, no alpha test), PRIM 0x58 (no fog bit), no
  0x01 → unlit (`FUN_004cf6f0` 594259, 594358, 594363–594364, 594431; `FUN_004abde0` 568938, 568949). Vertex colour = the
  segment colour (+0x34 = the static white `_DAT_0069c420`, 568926) × colour / 255 + 0.5, halved to the GS scale (594440–,
  594463). BF2 `com_sfx_muzzleflash` MTRL **0x10044**, diffuse white (`pxwb/mtrl2.py`): 0x04 translucent, **0x40 → 0x48**,
  vertex RGB = white × the HDR dim `DAT_007c5754` (0.5 in HDR levels, else 1.0) × colour (723449–723538); the verifier's
  PARTLY on C14/C18 concerns other flag combinations only (mode 1, render flags 0x20000000/0x400000, MTRL 0x08).
- [D] Per pixel: **dst += tex.rgb · (FlashColor.rgb / 255) · tex.a · (alpha / 255)**, alpha = min(1, 2t)·A/255, texture
  alpha scaled as every CLUT alpha ((A·0x81)>>8).
- **The texture** [V disc] (`disctex.py`): `meshmuzzleflash` (BF1 COMMON) / `com_sfx_meshmuzzleflash` (BF2 INGAME), 64×64,
  4-bit, 16 CLUT entries all RGB white with alphas {00, 94, 44, 22, c8, 69 (BF2 68), 10, 35, 08, 55, 7d, 19, ff, 2b, ab,
  03}: the shape is all alpha — a teardrop flame along U over the top half, a round glow bottom-left (mean alpha 38/255;
  `ps2cap/disc_bf1_meshmuzzleflash.png`). The model's 4 long planes carry the flame, the base cone the glow
  (`common_bf1/models/muzzleflash.glb`: 65 vertices, 96 triangles; BF2's: 113 / 184, the same planes turned).

## 5. Who draws it [V][O]

- B1: the weapon render slot +0x64 is called from exactly three places — the soldier render `FUN_002013a0` (reached from
  the renderable thunk `FUN_00210fb0`), which draws the current weapon at the pose's `hp_weapons` node (CRC 0x2b960099) ×
  the soldier matrix (114262–114306), and the first-person renders `FUN_001a4250` / `FUN_001a47d0` (47995, 48315). Only
  WeaponCatapult's wrapper calls `FUN_00285650` directly (212937). B2: the soldier render `FUN_00239bb0` calls slot +0x94
  (155363–155366 the current weapon; 155369 the dual-weapon class bit 55; 155377 another branch), the first-person
  `FUN_001c0688` / `FUN_001c0dd8`, and two wrapper weapons (311313, 338481–338502). `FUN_00287ba8` / `FUN_0032b0e0` have one
  caller each. [V] **No vehicle, turret or building code calls a weapon's render: their guns never draw this flash**, whatever
  their `MuzzleFlash` says.
- Thrown / placed weapons: weapon+0xbc ≠ 0 skips the render (209172); default 0 (instance ctor 207926), set only by the
  Detonator, Dispenser, Remote and Grenade types (verifier C7) — never by the cannon family.
- **Sabres** [V lead-read after the verifier ran, on its one-caller facts C12/C17]: WeaponMelee's swing (B1 `FUN_00290b98`
  = slot +0x74 of vtable 0x53c4c8, 217205) restarts the timer, but
  its render slot is its own — B1 +0x64 = `FUN_002911f0` (word 0x53c52c), B2 +0x94 = `FUN_0033e018` (word 0x6298cc) — and
  neither reaches the draw (`FUN_00285650`'s only direct caller is WeaponCatapult's `FUN_0028b0d8`; B2 `FUN_003278a8`'s only
  direct caller is WeaponCatapult's `FUN_00332090`, word 0x628584). The other 16 B1 weapon vtables keep `FUN_00285650`.
- [H] The soldier render's detailed path draws the weapon only for soldiers it renders in full; RAM shows fresh weapon
  blocks on the player and on soldiers out to ~60 m (BF1 NAB1), stale ones on the rest. The distance rule was not read —
  at the port's scale a 0.6-m flash beyond that is a few pixels; drawing it wherever the port draws the weapon is the call.
- [D] In the third-person chase view the player's own flash points into the screen and sits behind the helmet: invisible
  at natural size in every natural shot frame captured (`ps2cap/sheet_bf1_N.png`, `crop_natural_shot_vs_not.png`). What a
  player sees are the OTHER soldiers' flashes, side on.

## 6. What is never read [V][O]

- FlashWidth, FlashLightColor, FlashLightRadius, FlashLightDuration: in the weapon modules (B1 0x280000–0x2a0000, B2
  0x320000–0x350000) the only instructions touching those class offsets are the setter, the label ctor and the copy ctor
  (`checks.txt` §2); the verifier traced every other load of those offsets in both ELFs to its base and found none reaching
  a weapon class (C19). **PCSX2 knock-ins** agree: FlashWidth 5.0 on every class → the same flash (`sheet_bf1_D_width.png`,
  `sheet_bf2_BLW.png` W vs L); FlashLightColor green, radius 30 m, duration 2 s on every class → no green anywhere, BF1 KAS2
  and BF2 HOT1 snow (`sheet_bf1_L.png`, `sheet_bf2_BLW.png` L). The PC ODFs' flash light is a PC feature the PS2 never had.
- The preset's FlashLightColor and zero light radius/duration therefore have no effect either.

## 7. PS2 frames [V PCSX2] (`lead_tmp/fxmf/ps2cap/`, SW renderer, RAM patched live through `ps2live.Live`)

- `sheet_bf1_L.png` (**the reference frame**): BF1 KAS2 city hall, Republic rifle, FlashLength 1 s + FlashRadius 2.5 on
  the player's class: the camera pulled in behind a ghosted trooper shows the whole flash — a violet-white starburst, bright
  core, 8 spikes (the crossed planes end-on), FlashColor (80, 80, 255) additive over the scene.
- `sheet_bf1_C_big.png` (the same, normal camera: the starburst ahead of the rifle), `sheet_bf1_B.png` (FlashLength 1 s
  only, natural size: a small violet blob at the muzzle, seen when the rifle is raised), `sheet_bf2_BLW.png` (BF2 HOT1,
  Imperial rifle (255, 100, 100): a pink-white flash ahead of the rifle; B = draw only, L = + green light, W = + width).
- Natural fire: `sheet_bf1_N.png` / `sheet_bf2_N.png` — the RAM puts t = +1.000 on three BF1 and one BF2 frame; the flash is
  behind the head in all of them (§5).

## 8. What the port needs — FXMF build

1. **core::flash** (new, beside `core::laser`): `MuzzleFlashLook { radius, length, color: [u8; 4], model }` replayed from
   the weapon chain's PROPs parent first (the presets, FlashRadius, FlashLength, FlashColor, MuzzleFlashModel), label
   defaults (R −1 → none), the copy-ctor model default per game; FlashWidth / FlashLight* ignored with a comment (§6).
   Pure functions for the fade (alpha = min(A, 2A·t)) and the matrix (§4.2, cross products unnormalised).
2. **bake**: each weapon's fire point = the `FirePointName` node (default `hp_fire`) of its `GeometryName` skeleton, in the
   parent's frame (`bake::hardpoints` already reads skels), into the side weapon manifest; the flash textures keep their
   alpha (their only consumer is the additive flash material: BF1 0x84 Normal / BF2 0x10044 — `bake::alpha` rule 2 drops it
   today) → `rebake=common,side`.
3. **game**: on each `Shot` of a soldier's primary, end = sim time + FlashLength for that unit; each frame, for a drawn unit
   with t > 0: P = hp_weapons world × fire point, d = the unit's aim direction, the model `muzzleflash` /
   `com_sfx_muzzleflash` drawn additive, tinted, depth-tested, no depth write, unlit, unfogged. **Time t on the sim clock
   (the 30 Hz tick), not the interpolated render time**: a 0.025-s flash then shows for exactly the one tick of its shot at
   full alpha, as on the PS2 (FEEL_NOTES line). No flash for vehicle/turret/building guns or sabres; none for weapons
   without one (R ≤ 0).
   Replaces `fx.rs`'s `Kind::Muzzle` placeholder and `MUZZLE_FORWARD` / `MUZZLE_RIGHT` (bolt_vis's drawn start moves to
   the real fire point too).
4. **render**: an additive tinted model draw (depth test, no write) — the same pipeline FXP's GEOMETRY particles will want.
Acceptance: a data test reproduces §2's counts (68 / 182) and rows, §4.1's fire points (RAM values above) and the disc
texture's alpha (mean 38/255, not 255); a unit test of §4.2's matrix and §4.3's fade; a side-view sheet of the port's
flash next to `sheet_bf1_L.png`, viewed.

## 9. Still [H]

- The soldier render's distance/LOD rule for drawing the weapon (§5).
- What B2's weapon+0x68 ("the draw class") differs from weapon+0x64 for (equal at construction and in every RAM weapon).
- The far fade inputs of RedModel::Draw (`FUN_00479d90` / `FUN_00479da0`) — irrelevant at muzzle range.
- The first-person path (the port has no first-person view).

## 10. Checks run (§5 of the prompt) — `checks.sh` → `checks.txt` (663 lines)

1. **fn** — every cited B1/B2 line resolves to the function named (the three line ranges the draft had a few lines off —
   RedModel's fade, the Normal draw's TEST/ALPHA, the segment setup — were corrected before the verifier ran).
2. **absent** — the weapon-module field scans (FlashWidth / FlashLight*: setter + ctors only), every call through the
   weapon render slot (B1 3; B2 the 9 text hits + the register-form call at 155363, of which 152735 is a one-argument query
   on another object) and the trigger slot (B1 4; B2 the ordnance ctors), the key 0x8b83f9b7 (0 in both ELFs), the preset
   branch's accepted hashes (exactly three).
3. **twin** — every B2 function above opened and its callers walked (`callers`, the verifier's jal/data-word scan).
4. **word** — strings 0x53ad90 `muzzleflash`, 0x53adb0 `%f`, 0x53add0 `%i %i %i %i`; vtable words 0x53b288, 0x53ae88
   (0x2859f8), 0x53b29c (0x285650), 0x53adec (0x287a40), 0x558ff4 (0x496248), B2 0x627dfc, 0x627e3c, 0x627d5c; the preset
   floats as lui/ori pairs; the colours byte-built (asm); the hashes 0x9db10c98, 0xb3ea0ee8, 0x2b960099 as lui/ori.
5. **writers** — weapon+0xe0 / B2 +0xcc (the trigger, the reset), weapon+0xbc (defaults 0; grenade-type writers only),
   class+0x1a4 / +0x19c / B2 +0x1fc / +0x1f4 (setter, label ctor, copy ctor only).
- **RAM** — BF1 NAB1 (5 states, 200–226 weapons), KAS2 (31 states), BF2 HOT1 (4 + 17 states): class fields = the census,
  model pointers, |aim| = 1, fire points = the skeletons, t = +1.000 in shot frames.
- **PCSX2** — §7's knock-ins (FlashLength, FlashRadius → the draw; FlashWidth, FlashLight* → nothing).
- **Verifier** (one foreground Opus agent, `verify/CLAIMS.md` → `VERDICTS.md`): **18 SUPPORTED, 2 PARTLY, 0 NOT
  SUPPORTED**; the PARTLYs (C14 TEST/ALPHA for other modes and render flags, C18 MTRL 0x08 → ALPHA 0x64 and the dim's
  exceptions) are folded into §4.3; its notes added §5's third B2 call (155363) and WeaponCatapult's direct call, the
  uninitialised end time, and the 157 / 506 PROP counts of the unread smoke key.
