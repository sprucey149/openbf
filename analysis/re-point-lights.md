# OPUS RE (loop lane 3, 2026-09-30) - [O] UNAUDITED
# RE: BF2's local lights — the authored `lght` sets, the placed EntityLights, the command posts' light, the runtime lights; which draws take them and how (card BPNT)

**The question** (card BPNT; PCSX2 FINDINGS F48, F55): on the PS2, UTA1's lamp posts throw a red pool and wear a red
haze, its floors are yellow-lit, TAN1's corridor wall is pink, a soldier at his own CP is lit blue, and DEA1's CP shows a
red column. The port lights with the sun, Light2 and the ambient only. Which lights exist, which draws take them, and how?
Method: the loop's RE discipline, i.e. the five self-checks and one foreground Opus verifier on the claim list. Two
helper readers covered the EntityLight (`AGENT_A.md`) and the CP hologram (`AGENT_B.md`), and the lead re-read their
load-bearing lines. The PS2 was also taken apart: patched save states under PCSX2's SW renderer (§9).

Scratch, scripts, frames and sheets: `engine/artifacts/lead_tmp/bpnt/`. That covers `lights.py` (the RAM light list),
`lght_census.py`, `turn.py` / `mk.py` / `run.py` / `diff.py` (the knock-outs), `frames/sheet_*.png`, `verify/CLAIMS.md`
and `verify/VERDICTS.md`. B2 = `analysis/SWBFRNT2.ELF-decomp.c`. VU1 = the BF2 main program, read with
`scripts/tools/vu1dis.py`. "GS units" means 0x80 = 1.0. Tags: [V] read or measured, [D] derived, [H] guess.
Load-bearing claims are also [O].

## 0. The answer

**Four sources feed one list of render lights**, all of type omni (2), spot (3) or directional (1):

| source | where | Static? | colour → VU | range |
|---|---|---|---|---|
| authored `lght` Light blocks | every non-stub `lght` chunk of the map LVL (all at top level: every mode loads them) | per block (`Static`); most are | Color c (0..1): trunc(c·128), or (int)(c·255) with `PS2Overbright`, i.e. **×2** | `Range` |
| **EntityLight** entities (class label "Light") | attached to props through `AttachOdf` + hardpoint (UTA1 26, HOT1 88, KAM1 45, TAT3 120 …) | class `Static` | class Color bytes/255 × flicker I, ×128, never Overbright | `OmniRadius` (0 = no light) |
| the **command post's hologram** | every CP naming `HoloOdf` = `com_holo_controlzone` | never | the team-relation colour for the viewer: own team **(1,86,213)**, enemy **(200,100,100)**, neutral **(192,192,192)** (neutral locals (223,223,32)), × I (2 s fades, flicker dips) | 8 (`HoloLightRadius`) |
| runtime | bolts (`re-bolt.md` §5.2), explosion `LightFlash`, item lights (CTF flags → `flaglight`, mines) | never | per source (§4) | per source |

**Who takes them.**
- **Every model draw**, static (FUN_0057cf60) and skinned (FUN_001de2a8, soldiers), takes up to **4 omnis + 1 spot**.
  The spot counts as a point light, with no cone. They are chosen per draw by
  score = lum·min(1, r²/4d²)/max(1, e²/3600) among lights whose sphere meets the model's.
- A **prelit model** (any MTRL 0x200 segment) skips every Static light. The terrain takes only non-Static omnis, in its
  extra passes (`re-bolt.md` §5.3).

**The term** (VU1, per vertex, in GS units, added before COL0):

  colour += Σ C·mat·((1 + N·L̂)/2)²·(1 − min(1, d²/r²))²

C is the packed byte/128, and the sum is clamped at 255/128.

**Proved on the PS2 by knock-outs** (§9):
- **UTA1's yellow floor.** It is the Overbright yellow spots. At the walkabout pose the floor box drops from 103/93/55
  to 58/51/46 without model point lights, and **the port renders 57/46/40** there.
- **UTA1's red pool.** It is the lamp post's authored red spot, as a point light on the floor model.
- **TAN1's pink wall.** It is the **enemy CP's red light** on a prelit wall (the card's [H] "LREG or COL0" was wrong).
- **The blue soldier.** It is his own CP's light.
- What the port lacks is per-draw point lighting and the light sources that feed it.

**Not light** (§10). These are separate additive draws, and the "lamp glow" half of the card:
- the red haze at UTA1's lamp heads: an EntityLight **lens flare** plus a **light cone**;
- DEA1's red column: the hologram's **beam**, plus its spinning team icon.

GEO1's white-hot bunker strips are GLOW's self-lit MTRL 0x10 segments (`re-glow.md` §1.3, landed 375db4c), not a light.

## 1. The authored lights (`lght`) [V][O]

- **Parser.** FUN_00596978 (B2 728400) reads one Light block:
  - Type 0x5127f14d (1 dir, 2 omni, 3 spot); Color 0x3d7e6258 (floats as stored); Range 0xfadc0cd2;
  - Position 0x934f4e0a stored **(x, y, −z)** (728627); Rotation 0x21ac415f; Cone 0xdd404b6a; Texture 0x3c6468f4;
    PS2BlendMode 0x84e26526; Region 0x031afe7d (a directional light's region, → +0x60);
  - Static 0xd290c23b → word-0 bit 0x100 (728638/728642 → 728564);
  - CastShadow 0xe7e90aee → 0x200; CastSpecular 0x489162bb → 0x800;
  - **PS2Overbright 0x55b86d2c → 0x1000** (728709 → 728499 / 728513 / 728533).
- **The colour's packing** is the VU's input. Omni FUN_0059a6f8 730884–730896 (+0x64), spot FUN_0059bd78 731794–731806
  (+0xd4), dir FUN_00595a78 727835–727847 (+0x68):
  - without 0x1000: trunc(c·128) per channel;
  - with it: (int)(c·255) per RGB channel, A 0x80.
  - **So PS2Overbright doubles a light on models.** The terrain pass reads the floats (+0x1c..), not the packed word, so
    the doubling does not reach the terrain.
  - The port's `mungefs::light` drops the key as "editor-only". That is wrong.
- **Records.**
  - Omni: range +0x60 = +0x74, position +0x50 (copy +0x68..+0x70), colour floats +0x1c.
  - Spot: range +0x64 = +0xe4, position +0x80 (copy +0xd8..+0xe0), direction +0x70, cone sin/cos +0x50..+0x5c.
  - Sources: FUN_0059a550 / FUN_0059a6a8 / FUN_0059a6e0; FUN_0059bbc8 / FUN_0059beb0 / FUN_0059be98 / FUN_0059bee8.
- **Which chunks load.** Every `lght` chunk that holds lights or GlobalLights is at the map LVL's top level. The 181
  `lght` chunks inside `lvl_` mode sub-levels are 12-byte NAME stubs (verifier's scan).
  - So **every mode loads every authored light**.
  - RAM: UTA1's 50 omnis and spots all match their disc records by position, colour, range, Static and Overbright, in
    all 11 conquest states. TAN1's 44 lights are all held in its 5 conquest states (verifier).
  - GlobalLights' Light1 / Light2 leave the list: FUN_00596380 moves them to DAT_0060c6e0 / e4 (`re-model-lighting.md` §3).
- **Census** (`lght_census.py`; S = Static, P = PS2Overbright):
  - UTA1: 6 omni SP (255,100,100) r 6–11; 11 spot S (255,50,50) r 10–20 at the lamp posts, pointing down; 33 spot SP
    (243,253,68) r 8–14.
  - TAN1: 36 omni S (white, red (255,0,0), blue); 1 spot S; 6 region dirs S.
  - NAB2: 70 omni S. MUS1: 22 omni S, 2 textured spot projectors.
  - DEA1: 3 chunks, 86 lights, some non-Static and Overbright.
  - POL1: 4 omni S, 12 spot S. KAM1 / COR1: 2 omni S. DAG1: 3 omni S.
  - FEL1: 1 omni S, 2 spot S, 6 dir SP. GEO1: 2 spot S.
  - END1, HOT1, KAS2, MYG1, TAT2, YAV1: the GlobalLights pair only.

## 2. EntityLight (class label "Light") [V][O] — helper A, lead re-read 140382–140462, 140712–140790

- **Keys.** The class setter FUN_00229920 (B2 141739) reads (exact FNV names):
  - Color → +0x6c; OmniRadius 0xc2c1813a → +0xac; Static → +0x98 mask 0x1; ShadowCaster → mask 0x2;
    Synchronize → 0x10; SpecularCaster → 4; Type ("Spot" → 0x80, else 0x40);
  - FlareIntensity 0x633ed9d4 → +0x90; ConeLength / ConeWidth / ConeInitialWidth / ConeFadeLength / ConeFadeFactor;
  - FlickerType 0x3685bb77 → +0x9c; FlickerPeriod; FadePeriod; DrawDistance (200); Name; ProjectedTexture; FrameRate;
    SpotInner/OuterConeAngle.
- **The light** (ctor FUN_00227150):
  - an omni FUN_0059a550 unless Type "Spot" (140382–140395); no disc class sets Type, so all 63 light classes are omnis;
  - range = OmniRadius (140456);
  - class Static → render 0x100 (140458–140462);
  - position = the instance matrix's (FUN_00228770).
- **Registration** (FUN_00227e48):
  - the light is registered while active and I > 0, **and only if the range is > 0** (140733 / 140741–140742);
    `uta1_post_spot` (OmniRadius 0) makes no light;
  - colour floats = the class bytes/255 (140755), or lerp(black, C, I) for I < 1 (140775–140790);
  - never Overbright: the 0x1000 idiom occurs only in FUN_00596978, and no EntityLight in 176 states has it (verifier).
- **Placement.** Every shipped instance comes from a carrier class's `AttachOdf` + `AttachToHardpoint`: FUN_001d5260
  82916–82931 → FUN_001d4db8 82768 / 82823. There are 0 direct `wrld` placements.
  - A second create path, by name with an identity matrix (FUN_00229798, 141686), exists but no shipped ODF uses it.
  - The census matches RAM on UTA1 26/26, KAM1 45, HOT1 88, DEA1 29, TAN1 3, END1 4, FEL1 3 (`a_census.py`).
- **Flicker** (FUN_002278f8, 140589–140689): I is modulated by FlickerType.
  - Strobe 1: on a / off b.
  - StrobeRandom 2: dark at random, then ramps back up over 0.25 s.
  - StrobeFade 3: FadePeriod ramps.
  - Pulse 4: triangle wave.
  - Flicker 5: toward 0.5 + 0.5·rand.
  - The random is the global LCG DAT_0073d868, two steps per update per light. I scales the light, the flare and the
    cone; I ≤ 0 unregisters them.
- **UTA1**:
  - `uta1_post_flare`: Static omni (90,20,5) r 1, 11 instances. Knocking its light out changes nothing visible.
  - `uta1_bldg_tower_flicker`: non-Static (230,230,255) r 12, StrobeRandom 0.75, Synchronize; 4 at y 24.7.
  - `uta1_post_spot`: no light, only a light cone (§10).

## 3. The command post's light — the hologram's omni [V][O] — helper B, lead re-read 129216–129232, 129600–129635, 366990–367015, 712400–712418

- **Creation.**
  - `FUN_00354b08` links a CP to its building and builds the `HoloOdf` hologram at the building's `hp_hologram`
    (crc32 0xd1e4cc7a; 0.32 m up in RAM).
  - Its ctor FUN_00216948 makes one omni of range HoloLightRadius when HoloLightIntensity > 0 and HoloLightRadius > 0
    (129220–129227).
  - `com_holo_controlzone`: 8 and 1. It is the only hologram class with a light. Word 0 = 0x400: **not Static**.
- **Colour, every frame and per view** (FUN_002172a8, 129600–129632):
  - colour = lerp(black, T, I·HoloLightIntensity), where T = the viewer's team object +0x64 + 4·(hologram team);
  - the table is written by FUN_0036d9f8 from the Lua relations (366997–367012; constants FUN_0057ca38 712404–):

    | relation | colour |
    |---|---|
    | friend | 0xffd55601 = (1,86,213) |
    | enemy | (200,100,100) |
    | neutral | (192,192,192) |
    | neutral with side 5 ("locals") | (223,223,32) |

  - "Friend" = the local player's team. Split screen makes one light per view, each hidden from the other view.
  - No dispute colour, no blink. The ODF's own FriendlyColor / EnemyColor / NeutralColor keys have no reader in the ELF
    (verifier C21).
- **I, the hologram's intensity.**
  - 2 s fade in / out (HoloFadeInTime).
  - A random one-frame dip to U[0.4, 0.8] (HoloFlickerRate, HoloFlickerAlphaMin/Max).
  - Off during the last 2.2 s (HoloTurnOnTime) of a capture or neutralise, then the team and icon flip and it fades back
    in. Which of CP +0xa0 / +0xa4 is capture and which neutralise is [H].
  - Off when the CP building dies.
- **RAM** (UTA1, TAN1, DEA1): every CP light's colour = T·I exactly. Examples: DEA1 cp2 mid-dip I 0.488 → (0,42,104);
  TAN1 cp0 (the pink corridor) team 2 → (200,100,100).

## 4. Other runtime lights [V] — helper B §3 (constructors' complete caller lists)

- **Bolts / ordnance**: `re-bolt.md` §5.2. Built for the terrain by FXBL (b56c5d4); models are this card's.
- **Explosion LightFlash** (FUN_0030c8f8 → FUN_0030e108, 300250–300285 / 300803–300812): for an explosion class with
  LightRadius > 0 and LightDuration > 0, a non-Static omni.
  - colour = min(1, 20·LightColor′/255) per channel; radius 3 × LightRadius;
  - at the blast point + (0, 3, 0);
  - linear to black over LightDuration (FUN_0030e2e8).
  - RAM: 14 live flashes, r 15/21/30.
- **Items.** EntityItem ctor FUN_00224fb8: a `LightOdf` EntityLight (every CTF flag → `flaglight`: 225 255 255 64,
  OmniRadius 10, FlareIntensity 0.5), else LightColor/LightRadius (mines 247 56 65 150, r 3–5).
- Weapon `FlashLight*` keys are copied, never lit [H: no asm scan].

## 5. Filing and gathering [V][O]

- **Per view: FUN_00597918** (only caller FUN_00564638 698531). It walks DAT_007cd350 and skips:
  - a light whose word-0 low byte has the view's bit (the split-screen mask);
  - a light that fails its vtable +0x18 test (729112–729113; an omni tests its r + 10 sphere against the frustum).

  It then files:
  - type 2 onto DAT_007cd368: **non-Static at the head, Static at the tail** (729120 / 729136);
  - type 3 onto DAT_007cd380 the same way;
  - type 1 onto DAT_007cd398, where a Static light with CastSpecular (0x800) also goes to the head (729143).
- **Per model draw: FUN_00597e98**.
  - Callers: 712781 FUN_0057cf60 with (4, draw-flag 0x80 bit, model +0x48 & 1 = prelit, model +0xad); 90224
    FUN_001de2a8 with (4, 0, 0, 0).
  - It returns the default set when DAT_0060c2b5 == 0; that byte is 1 in the ELF and RAM, with no writer.
- **Directionals.** Light1 / Light2 (faded by the light regions, LREG) and then at most 2 from DAT_007cd398 in all:
  - a region-bound directional (Region key → +0x64) is weighted by FUN_005adc48 inside its region [H: the weight];
  - a prelit set stops at a Static one without 0x800.
- **Omnis**, from DAT_007cd368, at most 4:
  - a prelit set stops at the first Static (729642);
  - a light is a candidate when |c − L|² < (r_L + R)² (729647 / 729665);
  - score = (0.3R + 0.45G + 0.25B)·min(1, r²/(4d²)) / max(1, e²/3600), e = |L − eye| (729667–729696; asm 0x5988bc–
    0x598940: 0x3e99999a, 0x3ee66666, 0x40800000, 0x3e800000, 0x3991a2b4);
  - sorted insert, top 4 (729700–729714).
- **Spots**, from DAT_007cd380, **at most 1** (729745–729821):
  - position +0x80, range +0x64 (+ R), the light's vtable +0x1c cone test (729780), the same score;
  - a prelit set stops at the first Static.

## 6. The upload FUN_0057fd90 [V][O]

- Callers include FUN_0057cf60 712783 and FUN_003c3520 420216.
- **Gates** (715289–715290), each checked by the verifier for default and writers:
  - …
  - the set holds an omni or a spot (asm 0x57fef0);
  - DAT_0060c250 < 2 (the view count; ELF 1) or DAT_0060c2ca == 0 (ELF 0; split-screen writers only).
- **Filling the slots.** It sets VU flag 0x20 (715293) and fills up to 4 slots with the omnis' +0x64 records {packed
  colour, x, y, z, range} (715306). It skips the set's +0x94 projector and, for a **prelit set, every Static light**
  (715304–715305).
- **The spot's slot.** The spot's +0xd4 record goes into slot 0 if free, else the first free slot, else **over slot 3**
  (715318–715330).
- **Upload.** FUN_00497278 sends the four slots (715338). Unused slots point at the zero record DAT_007cd494.
- A prelit set uploads a **black ambient** (715402; `re-model-lighting.md` §3).

## 7. VU1: the point term [V][O]

- **Upload 0x56b.** Each record arrives as (x, y, z, 1/r²) (0x5a6–0x5ac: MULw r·r, DIV Q) and colour bytes × 1/128
  (I = 0.0078125 at 0x56d → vf31.x; 0x5a9 / 0x5ab). The records are transposed to VU[72..75] (colours) and VU[80..83]
  (x, y, z, 1/r² rows) by 0x510, and moved to model space at 0x73f (`re-bolt.md` §5.4).
- **Dispatch.** (flags & 0x78)/4 + 0x1d: 0x28 → 0x210 (points only), 0x38 → 0x222 (directionals + points).
- **Per vertex and light** (0x23a–0x2a0):
  - d² = |L − p|²;
  - att = (1 − min(d²/r², 1))² (0x246 / 0x24a / 0x24e / 0x252);
  - y = float(0x5EC00000 − bits(d²)/2) ≈ 0.5/d, up to ~9 % high (0x247–0x24f);
  - s = 0.5 + ((L − p)·N)·y ≈ (1 + N·L̂)/2 (0x24c–0x254);
  - **w = s²** (0x256);
  - colour += Σ C·mat·w·att (0x274–0x278), clamped at 255/128 (0x27f).
- **Then** the ambient pass and COL0 as `re-model-lighting.md` §2: × COL0/128 on a lit segment, + mat·COL0/128 on a
  vc-lit (0x200) one. mat carries the HDR 0.5, which the composite undoes.
- **In display terms** a white texel gains C·w·att. At the light (w = 1, att = 1) an unlit-by-anything surface shows C:
  (200,100,100) → ≈ +0.78/+0.39/+0.39 of full scale.
- **Skinned draws** (soldiers) take the same upload (FUN_003c3520 → FUN_0057fd90). The measured effect on the soldier
  is §9's; that their VU path shares 0x23a is [D].

## 8. Who takes what [V][O]

| draw | builder | omnis / spot | Static lights |
|---|---|---|---|
| static model, not prelit | FUN_0057cf60 | ≤ 4 + 1 | yes |
| static model with any 0x200 segment (prelit) | FUN_0057cf60 (+0x48 & 1) | ≤ 4 + 1 | **no** (no ambient either) |
| skinned: soldiers, and whatever FUN_001de2a8 draws | FUN_001de2a8 | ≤ 4 + 1 | yes |
| terrain patch (top layer, extra additive passes) | FUN_00599040(box, 4, 1, 1, 0) | ≤ 3 omnis; a spot only as a textured projector (`re-terrain-lighting.md` §5) | **no** (prelit) |

So **Static = "baked for the world, still lighting the characters"**:
- an authored or EntityLight Static light lights soldiers and non-prelit props only;
- a non-Static one (CP, bolt, flash, tower flicker) lights everything, the terrain included.

## 9. The PS2 taken apart [V measured]

`turn.py` boots a walkabout state (SW renderer, the README's isolation), holds the player, turns to a bearing and saves.
`mk.py` cuts copies with one family of lights zeroed:
- a light's colour floats and packed word are zeroed;
- or the code patch **0x57fed4 `lbu s0,161(v0)` → `daddu s0,zero,zero` (0x0000802d)**, which gives no model an omni;
- plus **0x57fecc → 0x0000982d**, which gives none a spot.

`run.py` boots each copy and grabs its frame; `diff.py` compares. Two controls were run each time; reload noise was
≤ 0.12 mean |d|.

| pose | variant | box | Δ RGB |
|---|---|---|---|
| UTA1 cp2 b000 (the walkabout pose) | no model point lights | floor (380,340,640,480) | 102.6/93.0/55.1 → **58.5/51.3/45.8**; the port renders **57.1/46.4/39.6** |
| same | the 33 Overbright spots zeroed | same | −32.8/−31.3/−7.0 |
| same | their 0x1000 cleared, packed re-made ×128 | same | −17.2/−16.2/−3.4 (**half**) |
| UTA1 cp2 turned to b180 | the lamp's red spot (255,50,50) r 16 zeroed | floor under the lamp | −19.7/−2.2/−1.8; the columns and lamp head too (`frames/sheet_u180_diff.png`) |
| same | the flare's omni (90,20,5) r 1 zeroed | — | nothing visible; the red haze stays |
| TAN1 cp0 turned to b270 | the CP light's record zeroed | — | 0.00: its owner rewrites it every frame (the saved state holds (200,100,100) again) |
| same | no model omnis (code) | wall (0,230,640,330) | **−17.4/−8.3/−8.3**, the pink gone (`frames/sheet_nopoint.png`) |
| UTA1 cp3 b000 | no model omnis (code) | the player (250,200,400,440) | 0/−7.9/−16.2, the blue gone (`frames/sheet_u3.png`) |

The library (HW Basic) and SW agree on the UTA1 floor box: 106.3/96.0/55.7 against 102.6/93.0/55.1.

## 10. The glows that are not light [V by the helpers; not re-read by the lead except the GS words]

- **Lens flare** (EntityLight FlareIntensity > 0; manager "LensFlare", FUN_00466d08; geometry FUN_00468238 / 468a28):
  - procedural and camera-facing: two 3-point stars (tips (2R, 4R) and (7R, 2R), turning with screen position) and a
    7-segment ring to 4R;
  - vertex colour RGB/2, alpha A·FlareIntensity/2 → /4 → 0; R = mean|ConeInitialWidth|/2 = 0.5 m by default;
  - ALPHA 0x48 additive, Z-tested GEQUAL, no Z write, no fog;
  - alpha × the visible fraction of a 2R test quad, read back through a 64×16 buffer (a soft occlusion);
  - culled beyond DrawDistance (200 m).
  - **UTA1's lamp haze** = `uta1_post_flare` (90,20,5,204·0.8).
- **Light cone** (ConeLength / ConeWidth; manager "LightBeam", FUN_00469628):
  - an additive untextured cone along the hardpoint's local +Y, length L·(1 − f) plus a fade tail L·f, widening from
    ConeInitialWidth/2 to ConeWidth/2;
  - alpha ∝ ConeFadeFactor·(1 − (v·axis)²);
  - `uta1_post_spot`: 8 m down, half-width 1 → 4, colour (60,30,27).
- **The CP hologram** (FUN_002186c8 → the same LightBeam renderer, 129937–129943; ALPHA 0x48, TEST 0x51001, PRIM 0x48 at
  528776–528779):
  - a beam cone 3.5 m tall, radius 0.5 → 1.0 m, in the team colour, alpha I·0.25·(1 − cos²);
  - the team icon `com_icon_*` spinning at HoloHeight, drawn unlit, translucent, alpha I·0.5;
  - a flare seen along the beam.
  - **DEA1 cp3's red column is the enemy CP's beam.**

## 11. What the port owes

- **BPNT build (this card)** — the lighting half:
  1. Bake every Light of every non-stub `lght` chunk: type, Static, **PS2Overbright**, colour, range, position (x, y,
     −z), spot direction and cone.
  2. Bake the EntityLights. A carrier class's `AttachOdf` + hardpoint gives each placed instance's light: OmniRadius,
     colour, Static, FlickerType / Period, Synchronize.
  3. At runtime add the CP hologram lights: r 8 at the CP + 0.32 m, the team-relation colour of the viewer. The 2 s fades
     and the flicker dips are optional polish (PROVISIONAL if built).
  4. Hand the renderer a per-draw light set by §5's gather (sphere test, score, top 4 omnis + 1 spot as a point; prelit
     draws skip Static).
  5. Add the term per vertex in the world and skinned vertex shaders: C = colour·(Overbright ? 255 : 128)/128, times
     texel and COL0 like the sun.
  6. Hand non-Static omnis (CP lights, flicker lights, flashes) to FXBL's terrain pass (card TDYN's build, folded in).

  - Acceptance: §9's boxes, re-taken by the port within ~10 levels: the UTA1 cp2 b000 floor ~103/93/55; TAN1 cp0_cp6con
    b270 the pink wall; UTA1 cp3 the blue soldier.
- **New card LFLR** — the glow half: EntityLight lens flares and light cones, and the CP hologram's beam and icon (§10).
- **Notes**:
  - **FXP5**: the explosions' LightFlash (§4) is a light on models and the terrain.
  - **LREG**: the region-bound directionals (§5) sit beside the light regions.
  - **GLOW**: GEO1's strips are its segments (§0).

## 12. What stays [H]

- FUN_005adc48's region weight for region-bound directionals; which DEA1 / TAN1 / FEL1 models sit in those regions.
- The skinned VU path's point loop is the static one [D]; only its effect was measured.
- The capture-versus-neutralise roles of CP +0xa0 / +0xa4.
- The draw order of the LensFlare (stage 6) and LightBeam (stage 3) managers.
- Weapons' FlashLight* keys (no asm scan).
- The uncracked keys 0x1de206a6 (the beam's base width) and 0x1e23f1a9 (the flare buffer's texture).

## 13. Verification

**Self-checks** (run and pasted in `claims_lead.md`, `AGENT_A.md` §Checks, `AGENT_B.md` §Checks):
- `re_check.py fn` on every cited line, all in the named functions.
- `word` for 0x57fecc / 0x57fed4 (the patch sites) and 0x60c238 = 1, 0x60c250 = 1, 0x60c2c8.. = 0.
- `writers` for DAT_0060c238 (0), DAT_0060c250 (FUN_00589e58), DAT_0060c2ca (FUN_00580d30, FUN_005898b0),
  DAT_0076c220 (FUN_0057ca38), DAT_007cd494 (0).
- `callers` for FUN_00597e98 (2), FUN_00599040 (2), FUN_0057fd90 (5), FUN_00597918 (1).
- Asm reads: the score constants (0x5988bc–0x598940); VU1 0x174, 0x210–0x2a0, 0x56b–0x5b5.
- Absence: the hologram colour keys (decomp both signs, raw words, lui halves: 0 hits); the Overbright idiom (3 hits,
  all in FUN_00596978).

**The verifier** (one foreground Opus agent, claims only, `verify/CLAIMS.md` → `verify/VERDICTS.md`): **18 SUPPORTED,
4 PARTLY, 0 NOT SUPPORTED**, all folded in:
- C4: the 181 stub `lght` chunks inside `lvl_` sub-levels; TAN1's 44 lights.
- C5: a Static directional with 0x800 files at the head; the per-view mask.
- C11: Static is class mask 0x1.
- C13: the second, unused create-by-name path.

It also checked every gate's default and writers in the ELF and in 9 states, and rechecked the VU constants under the
I-pairing rule.

## 14. Build notes (card BPNT build, 2026-09-30) — what the build read or measured beyond §1–§13

None of this section went through the verifier.

- **The spot's test against a model** (§5's "vtable +0x1c" = 0x59c3a8; the decomp has no function there, read from the
  asm with `scripts/tools/eedis2.py bf2 0x59c3a8 0x59c570`) [V asm].
  - Inputs: the model's sphere (c, R); the spot's apex V (+0x80) and axis D (+0x70); +0x54 and +0x5c, two trig values
    of half the OUTER cone (FUN_0059bff8, 731913–731924), whose product is used as the sine [D]; the `Bidirectional`
    byte +0xd1.
  - A `Bidirectional` spot flips D toward the model (0x59c3b0–0x59c418), so it never fails.
  - The routine builds its expanded-cone point from c instead of V (0x59c44c `vf3 = c − kD`, 0x59c454
    `vf1 = c − vf3`), so that first test always passes.
  - What is left: a model whose centre is BEHIND the apex inside the back cone — e = −D·(c − V) > 0 and
    e² ≥ |c − V|²·sin² — takes the spot only if |c − V|² ≤ R². Every other model in range takes it.
  - The spot's axis is taken as the directional light's reduction of `Rotation` [D]; UTA1's 11 lamp spots point down
    by it.
- **The model's sphere** (§5: FUN_0057cf60 712763–712781 gathers by the sphere at model +0x70). The build takes the
  `modl` chunk's `SPHR`, else the vis-box sphere [D]. It matters: UTA1's floor `uta1_bldg_macy_collision2` takes spot
  `macy16` (the one the PS2's floor shows) by `SPHR`, and `macy18` by the box centre 1.3 m away.
- **A textured light is the projector**, not a point light [D]: the upload skips the set's +0x94 projector (§6), and
  the gather keeps a light whose +0xc is set away from a model whose +0xad byte is set (729643, 729751; +0xc is
  written by the `lght` parser at 728556 — that it is the texture is [H]). The build drops every authored light with a
  `Texture` from the point list: DEA1 14, MUS1 2. With them in, MUS1's parity rose 7.92 → 13.48.
- **Measured** (`pose_render` at the stored PCSX2 poses over a scratch bake; `lead_tmp/bpnt/boxes.py`, `cmp3.py`):

  | pose, box | port before | port after | PS2 (library) |
  |---|---|---|---|
  | UTA1 cp2 b000 floor (380,340,640,480) | 57.1/46.4/39.6 | **105.6/90.1/51.6** | 106.3/96.0/55.7 |
  | UTA1 cp2 b180 floor (0,330,200,470) | 98.4/48.9/38.9 | 159.3/55.2/44.1 | 151/71/63 (the rest: the lamp's cone and flare, LFLR) |
  | TAN1 cp0 b270 wall (0,230,640,330) | 32.5/34.3/36.3 | 58.4/47.2/48.9 | 60.2/49.5/52.2 |

  - TAT2 authors no light: the posts' lights alone take `cp3_cp1_b000` from block error 47.6 to 7.1 (a blue wall) and
    `cp2_cp6_b000` from 37.5 to 14.0 (a red tower).
  - Parity over the 17 BF2 maps, 498 pairs, one binary: lights off 11.220 → on **10.427**. Down: TAT2 −5.58,
    TAN1 −2.94, UTA1 −2.43, KAM1 −1.69, MYG1 −0.66, DAG1 −0.52, GEO1 −0.51. Up: HOT1 +0.34, COR1 +0.25, MUS1 +0.18,
    DEA1 +0.13.
- **Closed 2026-10-03 by `re-vertex-colour-interp.md` (card BPRL, RE):** no draw skips the gathered set. The pen is
  drawn with flags 0x1000083 and the post's light in its set; a draw with 0x10 is unlit, not "lit by the last set".
  UTA1's pen floor and COR1's cp2 floor are the GS blending vertex colour in SCREEN space behind a clipper that cuts
  only at the guard band (a model of it gives the PS2's +1.2/+2.9 as +1.2/+3.0); DEA1's cp5_cp6 is a post the mission
  kills (`KillObject("CP6")`), which only the port's renders with no battle light. The text below is the question
  as BPNT left it.
- **Open** [H], card BPRL:
  - **Some prelit models take far less of a non-Static omni than §7 gives.** UTA1's pen floor
    (`uta1_bldg_lizardpen`, prelit_add, a vertex 0.35 m from the post's light): the PS2 gains +0/+1.2/+2.9 from its
    model omnis (`frames/u3__ctl.png` against `u3__code_57fed4_*.png`, box x 2–35 %, y 80–98 %), the port
    +0/+15.8/+38.1. DEA1 `cp5_cp6` (a blue wash the PS2 lacks) and COR1 `cp2_cp3` (a neutral post brightening the
    floor) are milder. TAN1's and KAM1's prelit walls and HOT1's corridors DO take the post's light at about the
    port's strength. Ruled out: the VU path (0x210 and 0x222 share the loop at 0x23a), the MTRL diffuse (ff ff ff ff
    everywhere, `re-model-lighting.md` "Diffuse"), the entity BASE (both `prop`). Unread: FUN_0057cf60's draw flags
    0x10 / bit 31 (no upload, the last set stays) and 0x4000000 (the default set, 712787).
  - **The viewer's team in a render with no battle.** The port lights the posts for the mission's team 1 at their
    world owners; the PCSX2 player is on team 2 on HOT1 and KAM1, so those pairs show the wrong relation colour
    (pink for blue).

## 15. The hologram's draw, read for the build (card CPHOL build, 2026-10-13) — [O] UNAUDITED

What the build read beyond §3 and §10. B2 = SWBFRNT2 export lines, B1 = SWBFRONT.

**15.1 The beam is a flat fan, not a cone** [V] B2 `FUN_00469628` 529100–529340 (the branch for length > 0), its tables
`FUN_004691f0` 528510–528600.
- Frame: x = side = normalize((base − eye) × axis), y = the axis, origin the base; z is constant (the fan is planar and
  turns about its axis with the eye).
- 22 vertices: 3 at y 0, x (i·0.6 − 0.6)·r0, alpha 0 (529250–529259); 6 at y 0.2·r0, x (i·0.4 − 1)·r0, alpha byte
  A·f·√(1 − (0.4 i − 1)²) (529264–529284; table 528581); 7 at y L, x (i/3 − 1)·r1, alpha A·f·0.25·√(1 − (i/3 − 1)²)
  (529287–529308; 0.25 = beam +0x18c; table 528587); 6 on the cap at x r1·c, y L + cap·√(1 − c²), c = (i + 0.5)/3 − 1
  pushed 1/18 away from 0, alpha 0 (529310–529322; 528570–528583).
- Strip of 34 indices (528529–528566): 8* 7* 2 6 1 5 0 4 3 | 9* 3* 10 4 11 5 12 6 13 7 14 8 15 | 21* 14 20 13 19 12 18
  11 17 10 16 9 (* = this vertex ends no triangle): 29 triangles.
- f = 1 − g², g = (v·axis)/|v| (r0 in place of |v| inside r0), v = base − eye; × (d_axis − near)/r_eye when the eye's
  distance from the axis less the near plane is under the cone's radius at the eye's height (0 when negative);
  × (end − d²)/(end − start) beyond start, start = min(the view's own far-fade start², 0.81 vis²), end = min(the
  view's far-fade end², vis²) (`FUN_00565120` / `FUN_00565130`; 529198–529222; the port takes the vis² terms).
  cap = soft + |g|·(r1 − soft): the export prints `FUN_0052a770()` (fabs, 657813) with its argument lost; the ELF
  words give it — $f12 is last written at 0x469c90 / 0x469ca4 (the divide that makes g) before the `jal` at 0x469e90.
- Every colour byte goes out halved, (b + 1) >> 1 (529229, 529281).

**15.2 BF2 modulates a white sheet, BF1 draws untextured** [V]
- B2 528783–528787: `FUN_005393c0(&h, 0x63ab08)` hashes the string at 0x63ab08 = "white.tga" (`re_check.py word b2
  0x63ab08`: 0x74696877 0x67742e65 = "whit" "e.tg") and the draw binds it (`FUN_00584a90(DAT_00720714, 0, 0)`).
- B1 `FUN_00386170` 374752–374775: the same GS record (0x100048, TEST 0x51001, 0x48) and, where BF2 binds its sheet,
  `FUN_0049e300(0, 0, 0)` (374857) — a bind of sheet 0. So a halved byte of 128 shows as 1.0 in BF2 and 0.5 in BF1.
- Measured on stored frames (`lead_tmp/cphol/tools/boxpair.py`): BF2 Death Star cp3 b000, the enemy column over the
  wall (box 290,80,380,200): PS2 132/88/100 against 46/46/60 without = +86/+42/+40 of (200,100,100). BF1 Kashyyyk
  Docks cp3 cityhall_v2 (box 545,110,595,200 against 470,110,510,200): 93/120/63 against 51/38/28 = +42/+82/+35 of
  (100,200,100) — half of BF2's per colour byte.

**15.3 BF1 draws the beam and the icon as BF2 does** [V] B1 `FUN_001e7d88` 95192–95400 (the twin of B2 `FUN_002186c8`
129858–130070): the per-view hide bit (+0xb8), the type-1 colour from the viewer's team table (+0x68 + 4·team),
alpha = I (+0xb0) × class +0x5c (HoloBeamIntensity), the call into the renderer `DAT_005194bc` (made with FNV
"lightbeam" 0xe8651328 at B1 363483), then the icon: class +0x64 (HoloImageIntensity), the 0.81 / 5.2631564 far
fade, the near fade, the model's draw with flag 0x10. Both discs' `com_holo_controlzone` author the same keys
(`b1dk/re/disc_classes.py holo`: height 3.5, size 2.5, beam 1.0, image 0.5, rotate 0.2, pop 0.01, flicker 0.01).
Seen: BF1 Endor cp2 postA_v1 and Kashyyyk Docks cp3 (stored frames) show the green column and the icon.

**15.4 When the post switches it off** [V] B2 `FUN_00357440` 352850–352854: on = 1 unless meter ≥ time − class +0x2c
(HoloTurnOnTime 2.2); 352930–352935 call `FUN_00218658` (off) / `FUN_00218550` (on); both turn a fade under way round
(clock = T − clock, 129790–129796, 129828–129834) and do nothing when the state is already theirs. The test is
reached with the neutralize meter +0xa0 at *(decompiler detail omitted)* (352848) and with +0xa4 on the jump from 352175; three
other jumps (352435, 352606, 352622) pass it by and leave the hologram on. Which time (class +4 CaptureTime, class +0
NeutralizeTime) reaches each jump is not traced: "an owned post tests its neutralize meter against NeutralizeTime, a
neutral one its capture meter against CaptureTime" is [D].

**15.5 The near plane** [H, fitted] The fan goes to the packed-strip draw with a box: x ±1.01·max(r0, r1),
y −0.01 … 1.01·(L + cap), z ±1 (529237–529238, `FUN_004976a8`). The stored PS2 frames show a frame without a beam
at the 12 poses whose eye is 0.28–1.11 m from a post's axis looking at it (a beam drawn by §15.1 would wash the
frame: parity +0.65 to +21 levels there), and show it from 1.79 m. "Not drawn where the box reaches the near plane"
fits all of them (the 1.11 m pose by 2 cm). What was read on the EE side: `FUN_004976a8` (555492) queues the box
(command 0x3fd), `FUN_00494af8` (553716) packs the strip, and the draw's own cull before them is a sphere against
the four planes (528844–528883), which lets a sphere that straddles the near plane through. The VU1 program that
takes the box is not in the export, so the rule is a fit to the frames.

**15.6 Verification** — one opus verifier, the 13 claims above with their lines only: 10 SUPPORTED, 3 PARTLY (the far
fade's two `min`s, the switch's paths, the fate of the box), 0 NOT; it recovered the cap's argument from the ELF
words and found the flare's fields: for g < 0 the beam's +0x124 alpha = A·g⁸·(+0x194) and +0x120 grows toward the
top's radius (529168–529188) — a flare seen only looking back along the beam.

**15.7 Still open** — the flare (§10, beam +0x120 / +0x124); the icon material's UV animation (the sheet
`com_icon_controlzone` is white with horizontal scan lines in its alpha, 163–255; the emblem is the model's
geometry); the sign of the icon's turn; END1's and HOT1's holograms on a building that is not the post; the port's
additive draws blend in linear light, the PS2 adds bytes: BF1's half-bright column reads about 60 % of the PS2's on
a dark wall (Kashyyyk +60 against +82, Endor +46 against +72).
