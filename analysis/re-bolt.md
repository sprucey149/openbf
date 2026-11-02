# OPUS RE (2026-09-30) - [O] UNAUDITED
# RE: laser bolts — the ordnance class's own draw (a core and a glow ribbon), its light, and HOT1's pink wash (card FXB)

Verified: one foreground Opus verifier, 25 claims: 23 SUPPORTED / 2 PARTLY (folded in) / 0 NOT SUPPORTED (§10).

Card **FXB** (M1, alpha), loop lane 2, effort max. Scratch (gitignored): `engine/artifacts/lead_tmp/fxb/` — `ramclass.py`
(class layout vs RAM), `ram_bolts2.py` (BF2 live bolts), `effective.py` → `effective.txt` (the showcase census after the
setter rules), `texnames.py`, `nibble.py` (the 4-bit texel check), `probe.py` / `probe2.py` (PCSX2: knock-outs, mid-fire
states), `vidstat.py`, `projbolts1.py`, `washcheck.py` (the wash model against the PS2 frame), `ps2cap/` (frames, sheets,
states), the helper reports `AGENT_A_bf1_draw.md`, `AGENT_B_bf1_light.md`, `AGENT_C_bf2_light.md`, `AGENT_D_bf2_draw.md`
and their scratch (`a_scratch/`, `b_light/`, `c_*.py`), `checks.sh` → `checks.txt`, `verify/CLAIMS.md` + `VERDICTS.md`.
B1 = `analysis/SWBFRONT.ELF-decomp.c`, B2 = `analysis/SWBFRNT2.ELF-decomp.c`. Tags: **[V]** read at the cited lines
(the lead re-read the load-bearing lines of §1, §3.1–§3.2, §5.2–§5.3; the verifier opened every claim of §10 —
§3.4 and §5.4's BF1 note rest on the helper reports alone); **[D]** derived; **[H]** hypothesis (PROVISIONAL in code);
**[O]** load-bearing for the build; "RAM" = read in a PS2 save state; "asm" = checked in the instruction bytes.

## 0. TL;DR — what the port needs

- **Keys** [V][O] (§1). The `bolt` / `laser` class (OrdnanceLaser) reads LaserTexture, LaserWidth, LaserLength, GlowLength,
  BlurLength, LaserGlowColor, FadeOutTime (+ BF2 EndLaserGlowColor); the bullet base reads LightColor, LightRadius (+ BF2
  EndLightColor, FadeLightColor). **LaserLength also resets GlowLength = 3·L and BlurLength = 15·L** (clamped to 128), so
  PROP order matters. The layout is checked field by field in RAM (BF1 NAB1 20 objects, BF2 HOT1 45, BF2 GEO1 40).
- **Draw** [V][O] (§3). A bolt is **two camera-facing tapered ribbons** built on the CPU, core first:
  - **core**: LaserTexture, **NORMAL** blend (GS ALPHA 0x44), half-width **w** at the head → **w/2** at the tail, over
    min(len, LaserLength) behind the head, plus a cap **w** ahead of the head; vertex colour white, alpha = fade;
  - **glow**: texture **`lightglow`**, **ADDITIVE** (0x48), half-width **2w → 1.6w**, over min(len, GlowLength), cap 2w;
    vertex colour = LaserGlowColor (authored alpha ~100/255, so the glow is faint and wide);
  - w = LaserWidth·(0.9·fade + 0.1); the texture's U runs along the bolt (0 at the cap tip, 0.25 at the head, 1 at the
    tail); depth tested, **no depth write, no fog, no lighting**, no distance fade; a nose-on bolt becomes a square blob.
- **Lifecycle** [V][O] (§4). The drawn length grows from 0 at the spawn point at the class Velocity up to
  max(LaserLength, GlowLength, BlurLength); BlurLength is otherwise invisible. At the impact the bolt stops, is drawn for
  two more updates, then vanishes — or, with FadeOutTime (snipers: 1 s), shrinks to 0.1·w and fades in place.
- **BF2 only** [V][O] (§3.5): the glow fades toward EndLaserGlowColor between the damage-falloff start and end (rifles:
  0.4 → 0.6 s of flight). **BF2's Republic rifle bolt is blue with red End colours (inherited from `com_`), so its glow
  and light turn red late in flight** (RAM). Vertex RGB is halved again in HDR levels (HOT1: on).
- **The light** [V][O] (§5). **BF1 bolts emit no light** (the fields are parsed and never read; its terrain light pass is
  compiled in and switched off). **BF2 bolts each own a non-Static omni light** (LightColor/255, range LightRadius, at the
  midpoint of the last step; deleted with the bolt). BF2's terrain adds one pass per light: the `com_sfx_omniproj` cos²
  blob projected straight down from 5 m above the light — **a disc of radius ≈ 0.98·(5 + dy) m whatever the LightRadius**,
  × max(0, 1 − dy²/r²), at vertex alpha 0.25: about **+32 red on the snow** under a rifle bolt 1 m up. **That is HOT1's
  pink wash** — proved on the PS2 by knocking LightColor out (§6) and predicted pixel by pixel from RAM (corr 0.93).
  BF2 models take the light too (per vertex, ≤ 4 per model).
- **Side finds** (§7): the port's **4-bit texture decode is nibble-swapped** — every 4-bit texture in the bake has each
  pair of horizontal texels swapped (new card **NIBL**); BF1 `purplelaser_D` is on no disc; key names `OrdnanceEffect`
  (0x3f30370c), `BonusColor` (0x21de3fab), `LightOdf` (0xe369687e); HOT1's HDR effect is on.
- **Cards** (§8): **FXB** → build (the ribbons); new **FXBL** (BF2's bolt light on the terrain); new **NIBL**; notes on
  TDYN, BPNT, FXP3.

## 1. The ODF keys, their setters and the defaults (both ELFs) [V][O]

Labels `bolt` / `laser` → OrdnanceLaser (`analysis/re-blaster-damage.md` §1): B1 ctor `FUN_0027c9d0` (class 0xf8 B), setter
`FUN_0027cae0` (B1:202429–); B2 ctor `FUN_0031ba10` (0x13c B), setter `FUN_0031bb60` (B2:310086–). Unknown keys fall
through to the bullet setter (B1 `FUN_002774b0`, B2 `FUN_00314b98`). Names: fnv20 (FNV-1a over byte|0x20), all recomputed
by the verifier.

| key (hash) | B1 off | B2 off | setter rule | ctor default |
|---|---|---|---|---|
| LaserWidth (0x6f179ab8) | +0xdc | +0x11c | float | 0 |
| LaserLength (0xba858240) | +0xe0 | +0x120 | min(v,128); **GlowLength = min(3v,128), BlurLength = min(15v,128)** | 0 |
| LaserTexture (0xb709b5e7) | +0xe4 | +0x124 | fnv20(name), the texture table key | 0 |
| LaserGlowColor (0x30580110) | +0xe8 | +0x128 | 4 bytes R,G,B,A as authored | white 0xffffffff |
| EndLaserGlowColor (0x266f54f7) | — | +0x12c | 4 bytes | white |
| GlowLength (0xc3a8b510) | +0xec | +0x130 | min(v,128) | (B1 unset, see below) / 0 |
| BlurLength (0x9d0fb7a4) | +0xf0 | +0x134 | min(v,128) | (B1 unset) / 0 |
| FadeOutTime (0x798dc484) | +0xf4 | +0x138 | 1/v if v > 0, else 0 | 0 |
| LightColor (0x746ce73e) | +0xd4 | +0x110 | 4 values: (R·A/255, G·A/255, B·A/255, 0xff); 3: (R,G,B,0xff); else unchanged | 0 |
| LightRadius (0x2776564d) | +0xd8 | +0x118 | float | 0 |
| EndLightColor (0x96e0ccb1) | — | +0x114 | as LightColor | 0 |
| FadeLightColor (0xa58acbd0) | — | byte +0xc1 | v ≠ 0 | 0 |
| OrdnanceEffect (0x3f30370c) | laser | laser | presets red/green/blue_blaster_bolt[_small] (width, length, texture, glow); the bullet setter swallows it; **no bolt class on either disc uses it** (it appears only on towcable/missile/grenade/sticky classes) | — |
| BonusColor (0x21de3fab) | — | ordnance base `FUN_00310620` +0x44 | BF2: the core colour while the shooter's team holds `team_bonus_advanced_blasters` (0xbb03442e) | white |

- Setter lines [V]: B1 LaserWidth 202512, LaserLength 202477–202488 (immediates `lui 0x4300 / 0x4040 / 0x4170` = 128 / 3 /
  15 at 0x27cbb0–0x27cbc8), LaserTexture 202462, LaserGlowColor 202504, GlowLength 202490, BlurLength 202468, FadeOutTime
  202452; LightRadius 198360, LightColor 198401–198416. B2 310109–310245, EndLaserGlowColor 310168; LightRadius 305278,
  LightColor 305324–305340, EndLightColor 305351–305366, FadeLightColor 305345–305347.
- Defaults [V]: the white is the engine's static white — B1 `_DAT_0069c420` (.bss; only writer the static initialiser
  `FUN_00492598`, B1:555048), B2 `_DAT_0076c200` (only writer `FUN_0057ca38`, B2:712404). B1's copy-constructor
  `FUN_0027ca48` does **not** copy +0xec/+0xf0 and the class clone memory is not zeroed (verifier C5, asm): a BF1 class that
  inherits from another bolt class and sets none of LaserLength / GlowLength / BlurLength / OrdnanceEffect would hold stale
  bytes there — no shipped BF1 class does (all 122 set LaserLength). BF2's copies every field (`FUN_0031baa8`).
- **Order**: the setters run per PROP in disc order, parent class first. `pistol_ord` authors LaserLength 0.75, GlowLength
  0.75, BlurLength 0.75, then GlowLength 3, BlurLength 5 → 3 / 5; `rep_weap_inf_remotedroid_blaster_ord` authors only
  LaserLength 0.75 → GlowLength 2.25, BlurLength 11.25 (RAM).
- **RAM** [V measured] (`ramclass.py`: every object with the class vtable B1 0x53a4f0 / B2 0x627218 at +0x1c, named by the
  fnv of its type at +0x14): BF1 NAB1 20, BF2 HOT1 45, BF2 GEO1 40 objects; every field equals the disc value after the
  rules above (e.g. BF2 rifle LightColor "253 43 43 150" → bytes (148, 25, 25, 0xff); `jedifighter_cannon` BlurLength 128).
  BF2 `tur_weap_hoth_dishturret_ord` is **four** class objects (its charge levels).

## 2. The census — six showcase maps (`effective.py` replays the setters; 40 BF1 + 50 BF2 bolt classes)

| game | class | texture | w | len | glow | blur | glow RGBA | light RGB | r | 1/fade |
|---|---|---|---|---|---|---|---|---|---|---|
| BF1 | all/imp/cis rifle, cis repeater | redlaser_D | 0.1 | 1 | 3 | 5 | 253,43,43,100 | 148,25,25 | 4 | 0 |
| BF1 | rep rifle | bluelaser_d | 0.1 | 1 | 3 | 5 | 92,136,250,100 | 36,53,98 | 4 | 0 |
| BF1 | all/imp/cis pistol | redlaser_D | 0.09 | 0.75 | 3 | 5 | 254,6,18,100 | 199,4,14 | 4 | 0 |
| BF1 | snipers (all/imp/cis/rep/tusken) | per side | 0.15 | 20 | 24 | 32 | per side, A 100 | per side | 4 | 1 |
| BF1 | AAT cannon | orangelaser_D | 0.35 | 6 | 18 | 90 | 250,170,5,100 | 0,0,0 | 5 | 0 |
| BF1 | X-wing cannon | orangelaser_D | 2.0 | 22.5 | 15 | 15 | 249,42,42,100 | 0,0,0 | 0 | 0 |
| BF2 | all/imp/cis rifle | com_sfx_laser_red | 0.1 | 1 | 3 | 5 | 253,43,43,100 → end 160,43,43,100 | 148,25,25 → 94,25,25 | 4 | 0 |
| BF2 | rep rifle | com_sfx_laser_blue | 0.1 | 1 | 3 | 5 | 92,136,250,100 → **end 160,43,43,100** | 36,53,98 → **94,25,25** | 4 | 0 |
| BF2 | pistols | com_sfx_laser_red/blue | 0.09 | 0.75 | 3 | 5 | 254,6,18,100 (no fade) | 199,4,14 | 4 | 0 |
| BF2 | snipers | per side | 0.15 | 20 | 24 | 32 | per side, A 100 | per side | 4 | 1 |
| BF2 | AT-ST / AT-AT head, tanks | com_sfx_laser_red | 0.35–0.75 | 6–15 | 18–45 | 90–128 | 249,42,42,100 | 0,0,0 | 5–7 | 0 |

- BF2 rifles fade (FadeLightColor 1) between age 0.4 and 0.6 s (class +0x34 / +0x38 in RAM = the damage-falloff pair,
  `re-blaster-damage.md` §7.3). The Republic rifle's red end colours are inherited from `com_weap_inf_rifle_ord` (RAM GEO1:
  EndGlow 0x642b2ba0, +0xc1 = 1) — the PS2 turns a clone bolt's glow and light red past 64 m.
- Vehicle cannons author LightRadius without LightColor: BF2 then makes no light (the alpha-byte gate, §5.2); BF1 makes
  none anyway. BF1 `cis_weap_inf_remotedroid_blaster_ord` names `purplelaser_D`, which no LVL holds (§3.3).
- Textures [V]: every name resolves in BF1 COMMON.LVL / BF2 INGAME.LVL (`texnames.py`) and is already baked
  (`engine/assets_baked/common_bf{1,2}/textures/`): 32×32, a soft horizontal ellipse, white-hot centre, coloured rim, alpha
  ≤ 204; `lightglow` (0x716bd81a) is white RGB with a soft teardrop alpha (max 210) peaking near U ≈ 0.28. All are 4-bit —
  see §7 before sampling them.

## 3. The draw

### 3.1 The call sequence [V][O] — B1 `FUN_0027c590` (B1:202239–202370), B2 `FUN_0031b258` (B2:309844–310024)

Class C = inst+0x30, next charge level N = C+0x2c (C itself without a chain), charge t = inst+0x34; lerp(x) = C.x + (N.x −
C.x)·t. Head H = inst+0x40 (B2 +0x50), dir = normalize(velocity inst+0xd0 / +0x110), len = inst+0x120 / +0x160, fade =
inst+0x124 / +0x164.
1. w = lerp(LaserWidth)·(fade·0.9 + 0.1) (B1:202316; B2:309955; 0x3f666666 / 0x3dcccccd in asm); Lc = min(len, lerp
   LaserLength), Lg = min(len, lerp GlowLength).
2. Glow colour G = per byte (int)(C·(1−t) + N·t + 0.5) of LaserGlowColor (B1:202288–202299); BF2 fade toward End (§3.5);
   then, if fade < 1, all four bytes × fade (truncated) (B1:202302).
3. `FUN_004af910` / `FUN_005941b8` (box H ± 2(len + 3w)): the 16-bit vertex quantisation frame (+ BF1's near/far bucket);
   its result is not tested — no cull, no fade (asm 0x27c858–0x27c868; B2 0x31b6b0).
4. **Core**: `… mode 5 (size w·0.5, point
   H − dir·Lc); colour BF1 (255,255,255,(int)(fade·255)) (B1:202335–202353), BF2 (inst+0x44 RGB, (int)(+0x47·fade)) (309975).
5. **Glow**: `… colour G
   (B1:202355–202367; B2:310006–310019, where G is also × inst+0x44/255).
6. Both passes run whenever Lc, Lg ≥ 0 — always.

### 3.2 The ribbon: `FUN_0047e108` / `FUN_00569328` modes 4 and 5 [V][O]
- **Mode 4 draws nothing** (jump tables B1 0x5588e0 [4] = 0x47f3a4, B2 0x64f730 [4] = 0x56a270, the common tail): it
  latches the head H, its half-width s, the halved colour and the unused second float (B1:544626–544642).
- **Mode 5** (B1:544011–544486; B2:701785–702106) builds ONE strip on the CPU (strip packet, not the VU1 sprite overlay),
  from the latched H (half-width s) to its own point P (half-width p); E = the eye (camera +0x30 / +0x60):
  - n = normalize((P−E)×(H−E)) (VOPMULA/VOPMSUB), or −camera-right when that is zero; u = normalize((H−E)×n) — in the view
    plane, pointing from H toward the tail.
  - **Cap** (ahead of the head): H−u·s+n·s (U,V 0,0), H−u·s−n·s (0,1), H+n·s (0.25,0), H−n·s (0.25,1).
  - **Body**: a fan round H (0.25, 0.5) over H−n·s, P−n·p (1,1), P+n·p (1,0), H+n·s — a trapezoid from half-width s to p.
  - **Nose-on**: if the eye's squared distance d² to the line (H, P−H) is below s², the body alpha becomes
    a_b = (int)(a·d²/s²) (B1:544141) and a mirrored cap behind the head, H+u·s±n·s → H±n·s, is drawn with a − a_b — except
    its (0,1) vertex, which the PS2 rebuilds from the full colour and so keeps alpha a (a PS2 quirk, both games; verifier
    C10). The body is skipped when a_b = 0, the mirrored cap when a − a_b = 0. Gates B1 0x522a04/08, B2 0x60c172/73 = 1,
    no writers.
  - Every vertex colour is halved to the GS scale, (b+1)>>1 (128 = 1.0). UV = (int)(u·2047).
- So the sizes are: core cap w, body w → w/2; glow cap 2w, body 2w → 1.6w. A bolt at len = 0 shows only the caps.
- [V RAM] BF1 NAB1: the last latch of the frame was a glow tail of size 0.16 = 2·0.1·0.8, colour 0x327d442e = halved
  (92,136,250,100), at a point exactly 3.000 m (= GlowLength) behind a live bolt's head (`a_scratch/ramdraw.py`).

### 3.3 GS state, colours, textures [V][O]
- Records (B1 `FUN_004b02f0` 572152–572169, selected at 572406–572422; B2 `FUN_00595020` 727641/727648, written by
  `FUN_005957a8`): **core ALPHA 0x100044** = (Cs−Cd)·As/128 + Cd, **glow 0x100048** = Cs·As/128 + Cd; both **TEST
  0x51001** (ATE with ATST NEVER + AFAIL FB_ONLY = no alpha test, ZTE, ZTST GEQUAL, **no Z write**) and **PRIM 0x58 → 0x5c**
  (TRISTRIP, Gouraud, textured, blended, **no FGE**: not fogged). TCC=1, TFX=MODULATE; texture alpha is uploaded as
  (A·0x81)>>8 (disc 255 → GS 128). So Cv = Ct·Cvtx/128, Av = At·Avtx/128 — no overbright.
- Per pixel [D]: core = lerp(dst, tex.rgb, tex.a·fade); glow = dst + tex.rgb·(G.rgb/255)·tex.a·(G.a/255). With the authored
  glow alpha 100 the glow peaks at 0.39 of the texture alpha.
- BF2: `FUN_005942f8` halves the vertex RGB again when `DAT_007c5754` < 0.99 (B2:726872) — the HDR switch: 1.0 at init,
  0.5 only in levels whose `Effect("hdr")` has Enable ≠ 0 (HOT1: on; RAM 0.5), 1.0 in split-screen. The HDR composite
  brings the frame back up (`re-model-lighting.md` §1), so on screen the bolt keeps its level.
- Missing texture: a hash with no texture binds the fallback record 0x69f6d0 (8×8 at VRAM block 0, TCC 0 — `re-fx.md` §6):
  BF1's remote-droid core draws as a near-opaque ribbon of whatever VRAM block 0 holds [H: frame-buffer pixels].

### 3.4 Visibility, order, caps [V by the helpers: AGENT_A §3/§8, AGENT_D §4; not load-bearing]
- The bolt is a scene renderable: a sphere at the head, radius 2·cap (B1:202111; B2:309720), frustum-tested; BF1 near pass
  up to 1000 m view depth (class render-info, RAM); BF2 main pass sorted far → near, far pass per the near/far scene ranges.
- Bolts sort with particle effects, back to front (BF1 key view depth + radius); within a bolt, core then glow.
- Each bolt costs 4 of the 2000 `FUN_0047e108` / `FUN_00569328` calls per viewport (shared with particles).
- In split-screen every ribbon faces its own viewport's eye (E is read inside each mode-5 call).

### 3.5 BF2 differences [V][O]
- Core colour = instance word +0x44, written only by the ordnance base ctor `FUN_0030eef8` (301505: the static white;
  301544: C+0x44 = BonusColor when `FUN_0034b1b0(0x69a248, team, 0xbb03442e)` — the shooter's team holds
  `team_bonus_advanced_blasters`, verifier C9). Every live bolt in RAM: white. Only the shotgun/pistol tracers author
  BonusColor (240,0,0,255).
- **End-colour fade** (B2:309912–309936): if C+0xc1 (FadeLightColor) and age = inst+0x40 (seconds of flight, frozen at the
  hit; RAM 0.0333 s for a bolt 5.34 m from its spawn at 160 m/s) > C+0x34: f = C+0x38 > 0 ? clamp((age − C+0x34)/C+0x38,
  0, 1) : 1; G = (int)(G·(1−f) + End·f + 0.5) with C's own End (not charge-lerped). Before the start nothing changes
  (Ghidra's `goto` there is a branch-likely delay slot).
- The glow bytes are then × inst+0x44/255 (+0.5) (B2:309995–310005).

## 4. Lifecycle [V][O]
- **Spawn / ctor** B1 `FUN_0027c2c8` (202072–202114), B2 `FUN_0031af80` (309678–309724): fade = 1, spawn point stored
  (+0x110 / +0x150), len = 0, hit = 0, cap = max(lerp LaserLength, lerp GlowLength, lerp BlurLength); the renderable radius
  is 2·cap. RAM: six BF1 bolts in flight (NAB1) with |vel| 80, len 5 = cap, fade 1.
- **Update** B1 `FUN_0027c478` (202170–202232), B2 `FUN_0031b130` (309779–309820): in flight, the move (the one spawn ray
  of `re-blaster-damage.md` §5) then len = min(cap, len + dt·Velocity) — the class Velocity, not charge-lerped — or
  min(cap, |pos − spawn|) once the move has reached its end (+0xfc). The move reaching the precomputed impact stops the head
  there; the next update calls the hit (`FUN_0027c410` / `FUN_0031b0c8`: hit = 1, len = min(cap, |pos − spawn|)).
- **After the hit**: if C.FadeOutTime⁻¹ (+0xf4 / +0x138) > 0, fade −= dt·that and the bolt lives while fade > 0 (it stays
  put, w → 0.1·LaserWidth, core alpha → 0, glow ~fade²); else the next update deletes it. [D] So an ordinary bolt is drawn
  at its impact point on two more updates at full strength.
- BlurLength only enters the cap: it bounds how long len may grow (the drawn lengths are clamped to LaserLength and
  GlowLength), the cull radius and the sort key — it has no other effect on the picture (search-verified: no other reader
  of class +0xf0 or instance +0x120/+0x12c).
- BF2 extras: the update is skipped during network re-simulation (`DAT_00600561`); the spawn pool keeps a 20-slot reserve.

## 5. The light

### 5.1 BF1: none [V][O]
- The class fields +0xd4 / +0xd8 (and LightOdf 0xe369687e → +0xd0) are parsed and copied and **never read**: an ELF-wide asm
  scan following class pointers loaded from instance +0x30 (and weapon +0x40) finds no load of +0xd0..+0xdb; its positive
  controls find the laser's own fields (verifier C17 repeated it with its own scanner: 0 hits). The instance's light slot
  +0x100 is only ever stored 0 (B1:197541); the update / dtor use it only if non-zero (197681 / 197590). RAM: +0x100 = 0 on all
  six live bolts; no light on BF1's list sits at a bolt.
- BF1's terrain has an additive light pass (`FUN_00487908`, up to 4 lights) but its gate `DAT_005229b4` is 0 in the ELF with
  no writer (B1:549303–549315; 0 in all 79 BF1 save states) — `re-terrain-lighting.md` §3's "BF1 has no lit branch" holds in
  effect.
- What lights the scene near BF1 hits is the impact explosion's `LightFlash` (explosion class LightColor / LightRadius /
  LightDuration, fading linearly to black, models only): 17 BF1 bolt types name a lit explosion (vehicle / walker cannons,
  orbital salvo); no infantry rifle or pistol has an impact explosion (RAM +0x38 = 0). → a note on FXP3.
- PS2 (§6): zeroing every BF1 laser class's LightColor changes nothing measurable.

### 5.2 BF2: the bolt's omni light [V][O]
- **Created** by the bullet instance ctor `FUN_003130d8` (B2:304080–304106) when C+0x10c (LightOdf) = 0, LightRadius > 0 and
  the LightColor alpha byte ≠ 0 (a class that authors no LightColor gets no light — RAM: the snowspeeder recoilless): a light
  entity (pool 0x689520) with colour = LightColor, radius = LightRadius, at the bolt; activated. (A network gate before it —
  `DAT_00600548/549/54f/560/562`, ELF 0/0/1/0/1 — always passes offline.)
- **The render light** [V]: type 2 = omni (`FUN_0059a550`, 730769), **not Static**; colour floats +0x1c.. = bytes/255;
  position +0x50; range +0x60 = +0x74 = LightRadius exactly (`FUN_0059a6e0`, 730863–730865).
- **Follows the bolt** at the midpoint of its last step, (prev + pos)/2 (`FUN_00313600` 304330–304342 — half a step behind
  the head; RAM: exact). With FadeLightColor, its colour lerps LightColor → EndLightColor by the same f as the glow
  (304347–304385). At the hit `FUN_00313c40` moves it to hit + 0.5·normal (304541–304551).
- **Removed** with the bolt: for infantry bolts (no FadeOutTime) the impact light lasts one frame [D].

### 5.3 BF2: the terrain pass — HOT1's pink wash [V][O]
- `FUN_0056f208`'s lit branch (B2:705214–705340) draws, on a patch's top layer, one pass per omni light of the patch's set
  (at most 4 − used slots = 3 with no projector; no shipped map has one): texture **0x36462699 = `com_sfx_omniproj`** (BF2
  CORE.LVL; not baked), descriptor 0x76c0c0 = **ALPHA 0x148** (Cs·As + Cd, additive), **TEST 0x53001** (colour only, depth
  tested, no depth write), CLAMP 5, no fog.
- Colour = light RGB × HDR × **f**, f = max(0, 1 − dy²/r²), dy = light.y − terrain height at its (x, z) (`FUN_0055ebc0`), →
  bytes ×128 (0x80 = 1.0); **vertex alpha = 0x20 (0.25)** from the word at 0x60c1b0 (ELF, no writer); no COLR, no layer weight.
- **Projection** (`FUN_0056ee88`): from 5 m above the light straight down (−Y), a 90° frustum with the 0.5 bias:
  s = 0.5 + 0.5·(L.z − P.z)/z′, t = 0.5 + 0.5·(P.x − L.x)/z′, z′ = L.y + 5 − P.y; CLAMP. With `com_sfx_omniproj` = 64×64 grey,
  T(ρ) ≈ 255·cos²(π/2·ρ/0.98) (ρ in half-widths), the lit area is **a disc of radius ≈ 0.98·(5 + dy) m** — its size does not
  depend on LightRadius, which only gates the patches and dims with height.
- On screen [D] ≈ c·f·T/4 per channel (the HDR ×0.5 is undone by the composite): a rifle light (148,25,25) at 0.5 m → +34 R,
  +4 G/B at the centre, radius 5.4 m; at 2 m → +26 R, 6.9 m; above ~3.9 m → nothing.
- **Which lights**: per view, non-Static omnis are filed at the head of the list; the terrain gather `FUN_00599040`
  (730351–730416) keeps a light if the squared distance from the patch box ≤ r² and ranks by (0.3R + 0.45G + 0.25B)·min(1,
  r²/4d²)/max(1, e²/3600) (e = distance to the eye); top 4, the terrain draws the first 3. [D] A 4 m light reaches only the
  patches within 4 m, so a disc crossing into a patch farther than 4 m away is cut at the patch edge.

### 5.4 BF2: models [V]
- Static and skinned models gather ≤ 4 of the same lights (`FUN_00597e98` from `FUN_0057cf60` 712781 and `FUN_001de2a8`
  90224; sphere test |c − L|² < (r + R_model)²); a non-Static light reaches prelit buildings too. Per vertex (VU1):
  colour += C·mat·((1 + N·L̂)/2)²·(1 − min(1, d²/r²))² (a fast rsqrt, ≤ ~9 % high) — BPNT's model path.
- BF1's model point term (for its explosions' LightFlash) is ((1 + N·L̂)/2)⁴ (AGENT_B §2.2, not re-read by the lead).

## 6. The PS2 taken apart (this session) [V measured]
- **Knock-outs** (`probe.py`, ci.lock; the FXP spawn states, pitched down, rifle held 2 s; each variant a fresh boot; live
  RAM writes to every laser class): ctl, **nolight** (LightColor [+ BF2 EndLightColor] = 0), **noglow** (LaserGlowColor
  [+ End] = 0), ctl2. `vidstat.py`, pink = R − (G+B)/2 over the ground (30 fps recordings):

| BF2 HOT1 | ctl | ctl2 | noglow | **nolight** |
|---|---|---|---|---|
| frame-mean pink | 1.90 | 1.76 | 2.08 | **−0.20** |
| peak | 9.8 | 9.1 | 9.7 | **−0.1** |
| ground share > 6, max | 69 % | 66 % | 69 % | **5 %** (HUD) |

  **The wash is the light, all of it; the glow adds none** (`ps2cap/sheet_hot1_knockout.png`, `hot1_pinkmap_f73.png`); the
  player's own model, ~8 m from the impacts (outside the 4 m radius), shows none. **BF1 NAB1**: nolight changes nothing
  (ground red index 5.31 / 5.73 ctl / ctl2 vs 5.74 nolight, within the spread); **noglow removes the red soft halo** round
  each bolt head, leaving a small white-pink core streak (`crop_nab1_bolt_knockout.png`).
- **The wash model from RAM** (`probe2.py`: save states taken while firing; `washcheck.py`): HOT1 mid3 holds the player's
  bolt light just after it hit the snow at (6.15, 0.52, −1.08) (dy 0.52, f 0.98) and a CP light; casting every ground pixel
  of the state's own frame onto the RAM heightfield and summing §5.3 over the two lights predicts the frame's pink with
  **correlation 0.933, slope 0.91** over 125,600 pixels (binned predicted → measured: 2.6 → 2.8, 7.2 → 7.7, 14.3 → 13.4,
  25.1 → 20.1) — `ps2cap/bf2_hot1_mid3_washcheck.png`. (mid2's frame shows a wash whose one-frame impact light was already
  gone from RAM — consistent with §5.2.)
- **The ribbon from RAM** (`projbolts1.py`): NAB1 mid3's player bolt at 14 m depth projects a glow of half-width 10.3 px; the
  frame's red blob is ~20–25 px across, 7 px along the bolt from the predicted head (about one frame of travel)
  (`ps2cap/bf1_nab1_mid3_boltcrop.png`).

## 7. Side finds
- **The port's 4-bit texture decode is nibble-swapped** [V measured, cause D]. `mungefs::texture::unpack_4bit` takes the high
  nibble first; the PS2 (GS PSMT4) puts the left texel in the low nibble. Test (`nibble.py`): swapping every horizontal
  texel pair of the baked mip 0 makes 191 of 205 BF1 COMMON 4-bit textures (94 of 104 BF2 INGAME) markedly smoother and
  none rougher; for the 8-bit and 32-bit controls it makes 0 smoother and 34 / 5 rougher. HUD icons (e.g.
  `hud_target_flag_onscreen`) are zig-zagged in the bake and clean when swapped (`nibble_sheet.png`). Every 4-bit texture
  on every map is affected → card **NIBL** (value 5).
- Names cracked: `OrdnanceEffect` 0x3f30370c, `BonusColor` 0x21de3fab, `LightOdf` 0xe369687e, and the BF2 bonus
  `team_bonus_advanced_blasters` 0xbb03442e.
- HOT1 enables BF2's HDR effect (`Effect("hdr")` Enable 1 in HOT1.LVL; RAM 0.5) — re-fx §8's open question, for HOT1.

## 8. What the port needs — the cards
- **FXB** (build, M1, alpha): parse the laser keys in PROP order (the LaserLength rule) into the ordnance class; each bolt's
  drawn length, fade and (BF2) age as §4 — render-only, no sim change; a ribbon pass drawing §3's two strips per bolt with
  the baked laser textures and `lightglow`, NORMAL then ADDITIVE, depth test, no depth write, no fog, sorted with the other
  translucent draws; BF2's end-colour fade; the streak must grow from the weapon's fire point (the port spawns bolts at the
  chest: draw from the barrel hardpoint, keep the sim origin). `purplelaser_D`: draw `violetlaser_D` (PROVISIONAL, the
  intended art; the PS2 draws VRAM garbage). Pairs: the PS2 frames of §6 at the same poses.
- **FXBL** (new, build, M1, alpha): BF2 bolt lights on the terrain — a runtime omni-light list (bolt lights per §5.2), the
  §5.3 additive pass on the terrain's top layer (bake `com_sfx_omniproj` from BF2 CORE.LVL), the gather and 3-light cap.
  BF1: nothing. Acceptance: the HOT1 knock-out pair and `washcheck.py`'s numbers reproduced by the port.
- **NIBL** (new, build, M1, alpha): swap the 4-bit nibble order; rebake textures; every golden moves.
- Notes: **TDYN** (its RE items — the texture, the projection, the gather, the ordnance source — are answered here; the CP
  lights' owner is still open); **BPNT** (bolt lights are non-Static omnis that reach BF2 models, §5.4); **FXP3** (BF1's
  impact explosions carry `LightFlash`es, §5.1).

## 9. Still [H]
- What VRAM block 0 shows under the fallback texture (BF1 `purplelaser_D`); the GS COLCLAMP state.
- The draw order between BF1's near (0xd) and far (10) buckets; whether a freshly spawned bolt is drawn before its first
  update (only the caps, oriented by the fallback).
- `com_sfx_omniproj`'s own GS words (MODULATE and bilinear are [D]); the one-frame impact light assumes bolts sit on the
  generic update list; the meaning of BF2's network gates.
- The class +0x34 / +0x38 key names (0xa74561b2 / 0xee08d1ba, the damage-falloff pair); the VU1 point-light term was read,
  not measured on the PS2.

## 10. Checks run (§5 of the prompt) — `checks.sh` → `checks.txt`, the helpers' `agentd_checks.txt`, `b_light/checks.txt`
1. **fn** — every cited line (lead: 92 B1/B2 lines; helpers ~110 + 89 + ~120), each in the function this document names.
2. **absent** — the class light fields (asm scan through the class pointer, with positive controls; the verifier's own
   scanner: 0 loads of +0xd4..+0xdb); BF1 instance +0x100 (only the ctor's zero store); readers of class +0xf0 and instance
   +0x120/+0x12c; `purplelaser_D` (0 of 5884 texture names, 0 decomp hits); the preset key (`hashloc`: laser, beam and
   bullet setters only).
3. **twin** — every BF1 function above has its BF2 twin read (AGENT_D §9, AGENT_C §1), callers checked (`FUN_0031bb60` only
   from the class vtable; `FUN_003130d8` 7 callers, `FUN_00313600` 6).
4. **word** — 128 / 3 / 15 (`lui 0x4300 / 0x4040 / 0x4170`), 0.9 / 0.1 / 0.5 / 0.8 / 255 / 1/255 in both draws, `lightglow`
   `lui 0x716b / ori 0xd81a`, the preset floats, jump tables 0x5588e0 / 0x64f730, gates 0x522a04/08 / 0x60c172/73 = 1,
   0x5229b4 = 0, 0x60c1b0 = 0x20000000, the projector's 5.0 (0x56eed4), 1/3600 (0x599b90), the preset strings.
5. **writers** — DAT_0069c420 (1: the static init), DAT_0076c200 (1), DAT_005229b4 (0), DAT_00522a04/08 (0),
   DAT_0060c172/73 (0), 0x60c1b0 (0), DAT_007c5754 (init 1.0, HDR 0.5, split-screen 1.0); widescan / addrscan on each.
- **RAM** — class layout (3 states, 105 objects), live bolts (BF1 NAB1 6 / 34; BF2 HOT1 4 / 8), the BF2 light records (exact
  midpoints, range = LightRadius), HDR 0.5.
- **Verifier** (one foreground Opus agent, claims + citations only, `verify/CLAIMS.md` → `verify/VERDICTS.md`): **23
  SUPPORTED, 2 PARTLY, 0 NOT SUPPORTED**, both folded in (C9: the bonus is `team_bonus_advanced_blasters` only; C10: the
  mirrored cap's (0,1) vertex keeps the full alpha).
