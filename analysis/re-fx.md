# OPUS RE (2026-09-30) - [O] UNAUDITED
# RE: particle effects — the `fx__ ParticleEmitter` format, the PS2's emitter runtime and draw, and the ODF keys that spawn them (card FXP)

Verified: one foreground Opus verifier, 34 SUPPORTED / 6 PARTLY (folded in) / 0 NOT SUPPORTED (§12).

Card **FXP** (M1, alpha), loop lane 2, effort max. Scratch (gitignored): `engine/artifacts/lead_tmp/fxp/` —
`lvlwalk.py` (LVL walker + munged-config dumper), `census2.py` / `keycensus.py` (effects, emitters, types, blends,
every key on both discs), `ramcheck1.py` / `ramcheck2.py` / `ramcheck_bf2.py` (the template layout against the PS2's
RAM), `brute_xyz.py` (the key cracks), `ps2cap/` (PCSX2 frames of real impacts), the helper reports
`AGENT_A_runtime.md` (BF1 runtime), `AGENT_B_draw.md` (BF1 draw), `AGENT_C_odf.md` + `AGENT_C_weapons.txt` /
`_census.txt` / `_fxnames.txt` (ODF keys, spawn sites, the showcase census, the 427 effect names),
`AGENT_D_bf2twin.md` (the BF2 twin), `checks.txt` (every §5 self-check), `verify/VERDICTS.md`. Corpora: B1 =
`analysis/SWBFRONT.ELF-decomp.c`, B2 = `analysis/SWBFRNT2.ELF-decomp.c`.
Tags: **[V]** read at the cited lines (the lead re-read every [V] a helper reported that this document relies on);
**[D]** derived; **[H]** hypothesis (PROVISIONAL in code); **[O]** load-bearing for the build; "RAM" = read in the
PS2's memory from a save state; "asm" = checked in the instruction bytes (`scripts/tools/eedis.py`,
`scripts/tools/vu1dis.py`).

## 0. TL;DR — what the port needs

- **Format** [V][O]. A particle effect is one `fx__` chunk: `NAME` = fnv20(effect name) (FNV-1a over `byte | 0x20`,
  so `_` hashes as 0x7f — the same hash the ODF values use) + a munged config tree of `ParticleEmitter` scopes
  (`mungefs::config` already parses the tree). The discs use 63 keys; 56 are named, the other 7 belong to one rare
  Spawner shape (§2); the in-memory layout the parsers build is **checked field by field against the PS2's RAM** in
  both games (§4). BF1 128 effects / 701 emitters, BF2 292 / 1702 (§1).
- **Runtime** [V][O] (§5). Per emitter: bursts of `BurstCount` every `BurstDelay` after `StartDelay`, until
  `MaxParticles` particles have been emitted in total (a lifetime budget, −1 = forever); each particle is spawned from
  the Spawner (a Spread box or a normalised Circle direction → `Offset + dir·PositionScale`, velocity
  `dir·VelocityScale`, + inherited emitter velocity), then evaluated in closed form at its age through the Transformer
  chains (Position: Accelerate/Reach/Scale on velocity with trapezoid integration; Size: Reach/Scale/Move; Colour:
  Move/Reach on RGBA or HSVA), lives `Transformer.LifeTime`, and is multiplied by the per-instance emitter colour.
  BF1 draws them from only **20 random variants per spawner** (a cache keyed by particle index, quantised to 16 bits);
  BF2 seeds every particle on its own and clamps MaxParticles to 128 (§8).
- **Draw** [V][O] (§6). PARTICLE / ANIMATED: a screen-aligned square, **full width √2·size′**, rotated by
  π·frac(0.00314·rot); size′ = size × (1 + 0.2·clamp(lod, 1, 3)) — **never below 1.2× the authored size**. ANIMATED
  flips an atlas (BF1: a fixed 4×4 grid on the same sprite; BF2: a CPU quad 2·size′ wide, FrameSize = the cell edge in
  texels, `Looping`). BILLBOARD: a **world-axis** plane chosen by ⌊rot⌋ (BF1's are horizontal ground decals).
  SPARK: a camera-facing ribbon from `pos − vel·SparkLength·size-mul` to `pos`. STREAK: a ribbon through the
  emitter's particles. GEOMETRY: a model. EMITTER: a child emitter per particle.
  **NORMAL = GS ALPHA 0x44 (lerp by As), ADDITIVE = 0x48 (Cs·As + Cd)**, no alpha test, depth tested, **depth never
  written, never fogged**; vertex colour = c·128 (no overbright). Fade: by BoundingRadius class — far distance
  50 / 150 / 500 m, alpha ramps out over the last 20 %; **MinLodDist / MaxLodDist are parsed and never read**.
- **What spawns them** [V][O] (§7). Bolt impacts: `ImpactEffect{Soft,Rigid,Static,Terrain,Water,Shield}` on the
  ordnance, picked by the hit's collision class (soldier / vehicle / building / terrain / water / shield), spawned at
  the hit with world axes. Explosions: the explosion class's `Effect` (BF1 only within 60 m of the camera) or
  `WaterEffect` in water. Grenades/missiles: `TrailEffect` on the round. Vehicles: `DamageEffect`, `ChunkTrailEffect`
  / `ChunkSmokeEffect`, `ThrustEffect`, `WakeEffect`, `StompEffect`, `DeathEffect`. On the six showcase maps every
  rifle bolt impact is `ord_explosion` (BF1) / `com_sfx_ord_exp` (BF2) on every surface.
- **Not in `fx__`** [V]: the **muzzle flash** — `MuzzleFlash` only selects one of three hardcoded presets
  (small/med/large), a separate renderer → new card **FXMF**; and the **laser bolt** itself (the ordnance class's own
  draw; the port draws a box) → new card **FXB**.
- **Cards** (§10): **FXP1** mungefs + bake, **FXP2** the runtime + draw, **FXP3** the spawn hooks (sim events carry
  the ordnance class and the surface kind), **FXMF**, **FXB**; this card closes as the RE.

## 1. The chunk and the census

- [V][O] A particle effect is one `fx__` chunk: a `NAME` child holding a u32 = fnv20(effect name), then the munged
  config tree (`mungefs::config`: DATA = `[u32 fnv(key)][u8 argc][argc × 4-byte slots][u32 bloblen][blob]`, a SCOP after
  a DATA is its scope). Root records are `ParticleEmitter("<label>")` (fnv 0xcebb215f). Example, BF1 COMMON
  `ord_explosion` (NAME 0xc42b8278 = fnv20('ord_explosion'); plain fnv would be 0x387ac758): emitter `Sparks` (2
  ANIMATED `a_sparks3` particles, 0.2 s, colour Reach white → orange, ADDITIVE) with a nested `Flare` (1 `boom4`
  PARTICLE growing ×3 in 0.1 s, alpha 200 → 0).
- [V] The same tag also carries the map environment effects (`Effect("Water" | "ColorControl" | "Wind" |
  "Precipitation" | "Godray" | "FogCloud" | "Blur" | "MotionBlur" | "ScopeBlur" | "HeatShimmer" | "Lightning" |
  "WorldShadowMap")`, `Shield`, and top-level kinds 0xbdf357b3 / 0xe8651328 / 0x1f678786): B1's loader
  `FUN_0036bf48` routes `Effect` to a handler table by its string (B1:358151) and registers everything else in the
  effect table 0x647d88 (B1:358212; B2 FUN_004512b8, table 0x71fc88, B2:513046). Those are FXC's / WTHR's, not this
  document's.
- [D] Census (`census2.py`, all LVLs, an effect counted once per NAME):

  | | effects | emitters | PARTICLE | BILLBOARD | SPARK | GEOMETRY | EMITTER | STREAK | ANIMATED | no Type |
  |---|---|---|---|---|---|---|---|---|---|---|
  | BF1 | 128 | 701 | 518 | 50 | 37 | 39 | 48 | 2 | 1 | 6 |
  | BF2 | 292 | 1702 | 1104 | 113 | 175 | 212 | 85 | 6 | 7 | 0 |

  Blend: BF1 ADDITIVE 487 / NORMAL 197 / BLUR 13; BF2 1208 / 473 / 21 (BLUR only on PARTICLE). Emitters per effect: mean
  5.5 / 5.8, max 17 / 36.
- [D] Where they live: BF1 `COMMON/ENG/COMMON.LVL` 109 of 128 (every weapon, impact and explosion effect); maps 1–6
  each (map-tinted dust/stomp/spray: NAB1 `WalkerStomp`, `flyerspray`; TAT1 also `dustwake`); BF1 side LVLs none
  (`SIDE/CIS.LVL`'s one `fx__` is a `Shield`). BF2 `INGAME.LVL` 162 of 292; sides 1–8 (IMP 8, SNW 6, CIS 5), maps 1–15
  (UTA1 12, MYG1 13, HOT1 6), space maps 12–35. The helper census counts 25 references on the six showcase missions
  that resolve to a side-LVL effect and 37 to a map-LVL one (AGENT_C §4) — the bake must carry all three layers.
- [D] Every indexed argument on the discs is 0 (`Size(0, a, b)` … `RotationVelocity(0, a, b)`, all 2 403 emitters);
  every Transformer `Size` / `Color` track index is 0; no disc authors a Transformer `Rotation` track.
- [D] Names: 427 distinct NAMEs on both discs (BF1 157, BF2 341, 71 shared); 382 cracked (AGENT_C §5); the 45
  uncracked are never named by a class property (every class value was a candidate), so no spawn path loses one.

## 2. The key table (both ELFs)

Keys are FNV-1a over the lowercased name. The discs' particle trees use 63 distinct keys (`keycensus.py`). The PC
modtools' `.fx` vocabulary names 45 outright; 11 more were brute-forced from a particle dictionary
(`lead_tmp/veh0/brute.py`, `brute_xyz.py`; 2–3 words — a spurious 2-word hit ≈ 1e-5 per target): **TimePerFrame,
FrameSize, TotalFrames, BlurValue, BlurRes, Looping, WorldAligned, Vortex, StartX/Y/Z**. Unnamed (7): Spawner shape
type 4 (0xc091789a) and two of its X/Y/Z families (0xf102de8a / 0xf202e01d / 0xef02db64; 0xaa11aa04 / 0xab11ab97 /
0xac11ad2a — each triple differs only in the last character, shown algebraically) — 3 emitters, all on BF1 HOT1.

A **range** stores every `(a, b)` pair: B1 12 bytes `{base, span, random}` (FUN_002ba1f0: base = min(a,b)·scale, span =
|b−a|·scale, random = a≠b), B2 8 bytes `{base, span}` (FUN_0037b7f0). A one-argument record stores `{a, 0}`. Indexed
keys `Key(i, a, b)` go to `field + i·12` (B1 FUN_002ba2b0) / `field + i·8` (B2 FUN_0037b8a0). Colour keys are scaled by
1/255 (0x3b808081) at parse, nothing else is.

### 2.1 ParticleEmitter (B1 FUN_002ba2e8 @246942, template 0x224 B; B2 FUN_0037b8d0 @376046, 0x1b4 B)

| key | fnv | B1 | B2 | stored | default (ctor B1 FUN_002b1b38 / B2 FUN_003742a0) |
|---|---|---|---|---|---|
| StartDelay | 0x826cd95e | +0x08 | +0x0c | range | — |
| BurstDelay | 0x78fc3314 | +0x14 | +0x14 | range | — |
| BurstCount | 0x33cbb418 | +0x20 | +0x1c | range | — |
| MaxParticles | 0x865d5c88 | +0x2c | +0x24 | range (B2 halving, §3) | — |
| Red / Hue | 0x40f480dc / 0xe38d6359 | +0x38 | +0x2c | range ×1/255; Red sets mode 0, Hue mode 1 | 1.0 |
| Green / Saturation | 0x011decbc / 0xf5a2e289 | +0x44 | +0x34 | " (no mode write) | 1.0 |
| Blue / Value | 0x82fbf5cd / 0x425ed3ca | +0x50 | +0x3c | " | 1.0 |
| Alpha | 0x5d8b6dab | +0x5c | +0x44 | " | 1.0 |
| (colour mode) | | +0x68 int | +0x4c byte | never read at runtime (§5.8) | — |
| Size | 0x23a0d95c | +0x6c | +0x50 | range | 1.0 |
| Spawner | 0x4936726f | +0x7c | +0x5c | block §2.2 | |
| Transformer | 0xf1773bf8 | +0x1a8 | +0x130 | block §2.3 | |
| Geometry | 0x2a6d8591 | +0x1c8 | +0x150 | block §2.4 | |
| (render class, by BoundingRadius) | | +0x1fc | +0x190 | >20 / >5 / else → B1 0x5817d0 / 0x5817b0 / 0x581790 | 0x581790 |
| MinLodDist | 0x6245ae9e | +0x200 | +0x194 | f32 — **never read** (§6.8) | 10.0 |
| MaxLodDist | 0xb9d61dcc | +0x204 | +0x198 | f32 — **never read** | 50.0 |
| BoundingRadius | 0xcbbfdf2b | +0x208 | +0x19c | f32 | 5.0 |
| NoRegisterStep | 0xa7fc4de1 | +0x20c int | +0x1a0 byte | flag | 0 |
| WorldAligned | 0x33bbef79 | +0x210 int | +0x1a1 byte | flag | 0 |
| ParticleEmitter (nested) | 0xcebb215f | +0x214 | +0x1a4 | sibling template: a second instance of the same effect | 0 |
| SoundName | 0x7a5ff18f | +0x21c | +0x1ac | string | — |

### 2.2 Spawner (B1 FUN_002baab0 @247239; B2 FUN_0037bf00 @376291; offsets from the block)

| key | fnv | B1 | B2 | stored |
|---|---|---|---|---|
| Spread / Vortex / Circle / (type 4) | 0x494a00ce / 0xd8760e19 / 0x28217089 / 0xc091789a | type +0x6c = 1 / 2 / 3 / 4 | +0x48 | shape block at +0 |
| PositionX / Y / Z (in Spread, Circle, Offset) | 0x57d87d76 / 0x58d87f09 / 0x55d87a50 | +0 / +0xc / +0x18 (FUN_002bb9d0) | +0 / +8 / +0x10 (FUN_0037c918) | ranges |
| StartX/Y/Z + the two unnamed families (Vortex, type 4) | 0xe9b852e5 … | 9 ranges +0 … +0x60 (FUN_002bbbc0) | FUN_0037ca00 | ranges |
| Offset | 0x14c8d3ca | +0x70 | +0x4c | PositionX/Y/Z ranges |
| PositionScale / VelocityScale / InheritVelocityFactor | 0x9ff363c0 / 0x983c4168 / 0xb9a4157c | +0x94 / +0xa0 / +0xac | +0x64 / +0x6c / +0x74 | ranges |
| Size(i, a, b) | 0x23a0d95c | +0xbc | +0x80 | indexed range |
| Red·Hue / Green·Sat / Blue·Value / Alpha (i, a, b) | as §2.1 | +0xd0 / +0xdc / +0xe8 / +0xf4 | +0x90 / +0x98 / +0xa0 / +0xa8 | indexed ×1/255 |
| (colour mode) | | +0x100 int | +0xb0 byte | 0 RGB / 1 HSV — this one IS used (§5.8) |
| FadeInTime | 0x5083ecd9 | +0x104 | +0xb4 | f32 |
| StartRotation / RotationVelocity (i, a, b) | 0x3d4cef81 / 0x164b5138 | +0x10c / +0x118 | +0xbc / +0xc4 | indexed ranges, stored as authored |

### 2.3 Transformer (B1 FUN_002bb090 @247471; B2 FUN_0037c2d0 @376453) — the same 8 words in both

`[0]` Position chain · `[1]` Size chain, `[2]` count · `[3]` Rotation chain (0x21ac415f), `[4]` · `[5]` Color chain
(0x3d7e6258), `[6]` · `[7]` LifeTime (0x5212abee — every particle's life). A chain node: `[0]` next (`Next()`
0x5cb68de8, parsed recursively), `[1]` LifeTime (default 1.0, FUN_002ba1a0), `[2]` 1/LifeTime (0 when ≤ 0), `[3]`
vtable, then the op:
- Position (B1 FUN_002bbe80 @247995 / B2 FUN_0037cbd0): `[4..6]` vec3, `[7]` op **Accelerate (0x9d9f16f0) = 1, Reach
  (0x4cfd5078) = 2, Scale (0x82971c71) = 3** (scalar in `[4]`).
- Size, Rotation (B1 FUN_002bc1a0 / FUN_002bc448, op parser FUN_002bca10; B2 FUN_0037cd78 / FUN_0037ce90 /
  FUN_0037d148): `[4]` value, `[5]` op **Move (0x184b0014) = 1, Reach = 2, Scale = 3**.
- Color (B1 FUN_002bc6f0 @248306 / B2 FUN_0037cfa8): `[4..7]` RGBA ×1/255, `[8]` op **Move = 1, Reach = 2** (no Scale:
  the parser knows only those two).

### 2.4 Geometry (B1 FUN_002bb548 @247648, ctor FUN_002b6108; B2 FUN_0037c4d8 @376543, ctor FUN_00377b28)

| key | fnv | B1 | B2 | notes |
|---|---|---|---|---|
| Texture / Model | 0x3c6468f4 / 0xb08b665a | +0x04 name hash | +0x04 | B2 Texture also caches log2 of the texture's dimensions at +0x08/+0x0c |
| BlendMode | 0xfa784eab | +0x08 | +0x10 | **NORMAL (or anything else) = 0, ADDITIVE (0x06a6a0a5) = 1, BLUR (0x6bfbd198) = 2** |
| Type | 0x5127f14d | +0x0c | +0x14 | **PARTICLE (0x8a8743bf) = 0 (and any unknown), STREAK (0x596f63f3) = 1, EMITTER (0x576b09cd) = 2, ANIMATED (0x6f35dfbc) = 3, GEOMETRY (0x2a6d8591) = 4, BILLBOARD (0xe506fa64) = 5, SPARK (0x3f2f216a) = 6** |
| (model) | | +0x10 = model table 0x69c678 lookup, Type 4 only (FUN_002b6078) | | |
| ParticleEmitter | 0xcebb215f | +0x14 | +0x1c | the per-particle child (Type EMITTER) |
| TimePerFrame | 0x413495b0 | +0x18 (2.0) | +0x20 (2.0) | ANIMATED |
| FrameSize | 0xfaf59ea7 | +0x1c (32.0) — **never read** | +0x24 = log2(FrameSize) (5.0) | ANIMATED (§6.3) |
| TotalFrames | 0x55799c97 | +0x20 (4.0) | +0x28 (4.0) | ANIMATED |
| BlurValue / BlurRes | 0xfc1fd229 / 0x7971e4d6 | +0x24 / +0x2c int | +0x2c / +0x34 | BLUR |
| SparkLength | 0x530aaa4e | +0x30 | +0x38 | SPARK |
| Looping | 0x7593992d | not parsed (the hash is absent from B1's particle code) | +0x3c bool (default 1) | ANIMATED, BF2 |

## 3. Parse-time rules

- [V][O] **Continuous emitters.** After an emitter is parsed, `MaxParticles(−1, −1)` with `BurstDelay(0, 0)` becomes
  BurstDelay base 0.1, span 0.1 (random) — a burst every 0.1–0.2 s: B1 FUN_002ba2e8 247225–247230, B2 FUN_0037b8d0
  376077–376082.
- [V][O] **BF2 halves MaxParticles** (`max(x·0.5, 1)`, FUN_001de698 = max) when `*(u32*)(DAT_005fc4ec + 0x20) >= 2`
  and the base is > 0 (−1 is never halved) (B2 376186–376214). The count is 0 in its ctor, then set from DAT_00600b30
  (default 1; `ScriptCB_SetSplitscreen` sets 2 — verifier C32). The same word is a loop bound and a divisor across
  B2 (e.g. B2:69002–69009, 66186) — [D] the local viewport count, so this is a split-screen budget. B1 has no such
  rule.
- [V] BoundingRadius picks one of three render classes (> 20 / > 5 / else), B1 247110–247117, B2 376239–376249.
- [V] StartRotation / RotationVelocity are stored as authored; the only unit conversion at parse is colour ×1/255.

## 4. The layout, checked against the PS2's own RAM

- [V][O] BF1 NAB1 (`pcsx2/ref/states/bf1_nab1_cp1_cp2_postA_spawn.p2s`): 640 live emitter templates (vtable
  0x0053f1d8 at +4, byte +0 = 1); 608 match a disc emitter by Geometry texture hash. `ramcheck1.py`: the four timing
  ranges incl. the random word, Min/MaxLodDist and BoundingRadius identical on 608/608. `ramcheck2.py`: emitter Size,
  Spawner PositionScale / VelocityScale / InheritVelocityFactor / Size / Alpha / StartRotation / RotationVelocity /
  FadeInTime / colour mode / shape, Transformer LifeTime, Geometry Type / BlendMode / SparkLength / texture hash
  identical but for 2 fields on one template matched to a sibling sharing its texture. The verifier extended the check
  to every §2.1 / §2.4 field incl. the colours and flags: **608/608 equal**; +0x1fc agrees with the BoundingRadius
  thresholds on 640/640; +0x214 is null or a template pointer on 640/640 (`verify/VERDICTS.md` C8).
- [V][O] BF2 HOT1 (`bf2_hot1_cp1_cp6_postA_spawn.p2s`, `ramcheck_bf2.py`): 856 live templates (vtable 0x0062d308 at +8).
  **All 856 match a disc emitter on every non-texture field above** (23 field groups, 0 disagreements); 194 also match
  the texture hash; in the other 662 the texture word's upper 24 bits are rewritten after load (the low byte survives)
  [H: a resolved texture handle]. MaxParticles is un-halved in this single-player state, as §3 predicts.

## 5. The runtime (B1 read; the BF2 twin in §8)

Offsets: T = template, S = Spawner (T+0x7c), I = emitter instance, N = emitter-path node, p = particle (0x50 B).
- [V][O] **5.1 Stateless.** Each frame `FUN_002b2b10` (242027) rebuilds every live particle of an instance from its
  burst history: it walks the bursts from the oldest live one to now, re-spawns each particle (`FUN_002b79d0`, 242293)
  at age = now − burst time, and evaluates the Transformer at that age (`FUN_002b9468`, 242429). [D] A stateful port
  that spawns at the burst time and integrates the same closed forms gets the same particles.
- [V][O] **5.2 Bursts.** StartDelay is sampled once per instance (`FUN_002b1e40` 241766–241772). Per burst:
  BurstDelay re-sampled, the time to the next burst; BurstCount re-sampled and rounded, `(int)(x + 0.5)` (242232),
  clamped to what is left of MaxParticles (242234). **MaxParticles is a lifetime budget**, not a live cap: spawning
  stops when the emitted count reaches it (242211 *(decompiler detail omitted)*) and the instance dies when the retired count reaches it (retire `FUN_002b26a0` 241982); −1 = unlimited.
  At most 127 particles per instance per frame (the same line). An instance also ends after a Stop once no burst before
  the stop time is alive (`FUN_002b26a0` 241940). RNG: LCG `s' = s·0x19660d + 0x3c6ef35f`, float
  `((s1>>16 | s2&0xffff0000) >> 9 | 0x3f800000) − 1`.
- [V][O] **5.3 Twenty variants.** Spawned values come from a cache of **20 variants per spawner**, slot
  `(idx + I.offset) % 20` (245486), each filled once from the seed `(slot·10 + S+0x128) ^ 0xc71c2830` (245490) and
  stored as 16-bit integers: position and velocity ×100, size ×1000, StartRotation ×360, RotationVelocity ×100, colours
  as bytes (245491–245504; read back 245515–245530); the variant's InheritVelocityFactor is drawn unquantised
  (245505–245512). [D] StartRotation beyond ±91.02 wraps (mod 182.04); sizes beyond 32.767 m wrap.
- [V][O] **5.4 Spawn** (`FUN_002b7ef0` 245573 + `FUN_002b79d0`): shape Spread (1) → dir = the sampled (X, Y, Z) as is;
  Circle (3) → the same, normalised (`FUN_002b7948`); Vortex (2) / type 4 → `FUN_002b7208` [D: per-axis sines of the
  index — AGENT_A 2.3]; others → 0 (245595–245618). pos = Offset + dir·PositionScale (245620–245642); vel =
  dir·VelocityScale (245644–245662). Then into the world through the burst's path node: pos by rotation + translation,
  vel by rotation only; `vel += nodeVelocity · (InheritVelocityFactor + I.extra)` (245556–245560); size ×= the
  instance's emitter Size (245556).
- [V] **5.5 NoRegisterStep** spawns every burst from the newest node (the particles ride rigidly with the emitter);
  **WorldAligned** replaces the node rotation with identity (242271) [AGENT_A 2.10–2.11; 242271 re-read:
  `if (T+0x210 != 0)` swaps in the rows of DAT_0066ef20].
- [V][O] **5.6 FadeInTime is an emitter fade-in**: a burst emitted before `FadeInTime` of effect time has its alpha
  multiplied by `burstTime / FadeInTime` (242315–242325) — not a per-particle fade.
- [V][O] **5.7 The Transformer** (`FUN_002b9468` 246293; lifetime = `Transformer.LifeTime` for every particle). A node
  runs for its LifeTime with progress u = t/L, then hands on to Next with t − L. Position (`FUN_002b9610` 246392):
  h = min(u,1)·L/2; pos += vel·h; vel = op(vel); pos += vel·h; **Accelerate** vel += A·s·min(u,1) (A is the TOTAL
  change over the node, per second only when L = 1), **Reach** vel = lerp(vel, A·s, u) (A·s once u ≥ 1), **Scale**
  vel ·= lerp(1, k, u) (k once u ≥ 1); after the last node the particle coasts: pos += vel·(t − L) (246505–246510); s =
  the emitter Size. With no Position chain: pos += vel·age. Size / Rotation (`FUN_002b98a8` 246525, **asm**): Reach
  lerps, Scale multiplies by lerp(1, V, u), and **Move adds min(u, 1) — its authored value is ignored** (only tested
  for 0) — one BF1 emitter uses Size Move. The Rotation chain acts on the rotation VELOCITY. Color (`FUN_002b9b20`):
  **Move** c = clamp01(c + D·min(u,1)), **Reach** c = lerp(c, D, u), on all four channels (HSVA in HSV mode).
- [V][O] **5.8 Emitter-level colour.** The template's Red/Green/Blue/Alpha (or H/S/V/A) and Size are sampled once per
  instance; after the Transformer every particle's four channels are multiplied by them (242447–242466). The
  Spawner's colour mode (S+0x100) travels with the particle; HSV → RGB runs in the draw (`FUN_002b62a8`: 6-sector, H in
  [0,1)). The emitter-level mode word (T+0x68) is never read.
- [V] **5.9 ANIMATED frame** (242432–242441): `f = (int)(age / TimePerFrame); while (TotalFrames < f) f −=
  TotalFrames;` (strict `<`: a frame equal to TotalFrames stays) — f runs 0…N, frame 0 shown once (the original's
  off-by-one; the 4×4 atlas wraps f & 15).
- [V] **5.10 EMITTER** (Type 2, `FUN_002b34f8`, 242098): each parent particle (up to 31) replays the child template
  in a scratch effect along its path in 0.3 s steps, tinted by the parent's colour. Full read in §13.

## 6. The draw (B1 read; the BF2 twin in §8)

- [V][O] **6.1 PARTICLE and ANIMATED** are screen-aligned squares expanded in VU1 (overlay at VU 0x74e, MPG slot 1889;
  **asm/VU**): the corner offsets are `h·(±(S+C), ±(C−S))` with S = √2·sin(πr), C = √2·cos(πr) (the LOI polynomials
  0x78f–0x798), r = frac(0.00314·rot) (constant 0x3b4dc875 at 0x4b026c), h = size′/2 (the w word = size′·128 unpacked
  over STROW 32768 and `SUBi.w` 32768, 0x7b8) — the corners lie on a circle of radius size′, so **the square is √2·size′
  wide**, textured (0,0)–(1,1). [D] An authored rotation in degrees turns the sprite 0.5652° per degree and snaps back
  180° every 318.47 units.
- [V][O] **6.2 size′ = size × sizeMul, sizeMul = 1 + 0.2·clamp(lod, 1, 3)** (`FUN_002b8750` 245836–245846;
  … lod is §6.8's.
- [V][O] **6.3 ANIMATED** (BF1) is a **fixed 4×4 atlas**: u = (f & 3)/4, v = ((f >> 2) & 3)/4 (VU 0x7b9–0x7c6);
  FrameSize is never read (no load of T+0x1e4 in B1 240000–250000). BF1's one ANIMATED emitter (`a_sparks3`, 256²,
  FrameSize 4, 16 frames) is such a grid. BF2 reads FrameSize in texels — §8.
- [V] **6.4 BILLBOARD** is a **world-axis** square of half-width size′ (`FUN_0047e108` case 6, 544489–544604): k =
  ⌊rot⌋ picks the plane, φ = 2π·frac(rot) the in-plane angle; k = 4 is the horizontal XZ plane — BF1's billboards
  author StartRotation(0, 4.0, 4.9): ground decals with a random yaw (AGENT_B 1.7 lists every k).
- [V] **6.5 SPARK** is a camera-facing ribbon of 3 points: tail `pos − vel·L`, mid, head `pos`, L =
  SparkLength·sizeMul, half-width size′ (244993–245033). **STREAK** is a camera-facing ribbon through the emitter's
  particles in spawn order, half-width size′, U = the particle's rotation value (543776–544007); fewer than 3 → nothing.
- [V] **6.6 GEOMETRY** draws the model (table 0x69c678) at the particle, scaled by size (no sizeMul), turned about Y by
  the raw rotation in radians, under the emitter node matrix; missing model → nothing (245036–245124).
- [V][O] **6.7 GS state** (**asm**, records written only by `FUN_004b02f0`, 0x4b034c–0x4b04cc): {ALPHA word, TEST, PRIM}
  = NORMAL {0x100044 / 0x104044 with frames, 0x51001, 0x58}, ADDITIVE {0x100048 / 0x104048, 0x51001, 0x58}, BLUR
  {0x100040, then 0x100154}. ALPHA low byte **0x44 = (Cs − Cd)·As + Cd, 0x48 = Cs·As + Cd**; **TEST 0x51001 = ATE,
  ATST NEVER, AFAIL FB_ONLY, ZTE, ZTST GEQUAL**: no alpha cutout, colour written, depth tested, depth never written;
  **PRIM 0x58 has no FGE: particles are not fogged**. PARTICLE batches (≤ 300 per texture + blend) are heap-sorted
  back-to-front; other types draw in spawn order. BLUR = two passes over a screen copy (BlurRes picks the copy,
  BlurValue feeds the pass-2 UV matrix) — the heat-haze microprogram is [H].
- [V][O] **6.8 Colour and fades.** Bytes = (int)(c·255), then `(b + 1) >> 1` (543943–543946): 128 = ×1.0 under
  MODULATE, no overbright. Per effect (`FUN_002b8750`): far distance D by BoundingRadius — **50 m (≤ 5), 150 m (≤ 20),
  500 m (> 20)** (`FUN_001a90d8` / `9110` / `9148` via `FUN_001a9398` 51478–51480), × the current view's +0x90 factor
  (1.0 from the camera ctor, blended by a zoom in `FUN_001a9f20` — verifier C20); alpha ×= 1 − (d − 0.8D)/(0.2D) beyond
  0.8D (245812–245815); a near-scene fade unless flag 0x20000 (245817–245825);
  density: lod = max(0, (d − r − 20)·4/(min(D, 90) − 20)), particles kept by `idx & 3` in {1111, 1110, 1010, 1000}
  (DAT_00518408, re_check word), the next one to go fades by 1 − frac(lod). **MinLodDist / MaxLodDist: no reader**
  (every +0x200 / +0x204 hit in B1 240000–250000 is a write).
- [V] **6.9 Missing texture** (hash 0 or not loaded): a fallback record sampling VRAM block 0 (563013–563017) [H: what
  it shows]; the draw goes ahead.

## 7. What spawns effects — the ODF keys (both games; AGENT_C, load-bearing lines re-read)

The setters hash the ODF string with the same fnv20 and either resolve it at class load (L) or keep the hash and look
it up at spawn (H) in the effect table (B1 0x647d88, B2 0x71fc88). Full table: AGENT_C §2 (39 keys).
- [V][O] **Bolt / bullet impacts** (B1 `FUN_00276810`, B2 `FUN_00313c40`): the hit's collision class picks the slot —
  B1 (197939–198009): 2 or 0x10 → **ImpactEffectSoft**, 4 → **Rigid**, 8 → **Static** (its ImpactDecalStatic field is
  loaded but never drawn — the decal call fires only for 0x40, 198018), 0x40 → **Terrain** (+ ImpactDecalTerrain),
  0x80 → **ImpactEffectWater** via the splash helper `FUN_0037c920`, 0x800 → **Shield**, 0x100 → three hardcoded
  sparks, no ODF effect. The effect is looked up (198022) and spawned at the hit **with world axes** (identity
  rotation DAT_0066ef20, `FUN_0036d478`, 198038), or in the struck object's frame when there is one (198040–).
  [D] With the collision-layer bits (memory: B1 2 soldiers, 4 vehicles, 8 buildings, 0x40 terrain): Soft = soldiers,
  Rigid = vehicles, Static = buildings, Terrain = ground. B2: 2/0x20 Soft, 4/8 Rigid, 0x10 Static, 0x80 Terrain, 0x100
  water, 0x1000 shield → `ImpactEffectShieldSmall` / `Large` by target size < 16 (both setters write +0xfc — a PS2
  bug — so the ctor defaults `com_sfx_shieldimpact_sm/_xl` stand) (B2 304647–304744). Unauthored: B1 Terrain =
  'dirtspray', Water = 'watersplash_ord' (ctor 198246–198248).
- [V][O] **Explosions** (B1 `FUN_002701a0` 193756–193807): visible if within **60 m** (`… in
  water (pos.y ≤ water + 0.3, and DamageRadiusOuter +0x58 > 0) → `WaterEffect` + a ripple; else the explosion
  class's **`Effect`** at the blast matrix (193807); beyond 60 m only the effect's SoundName plays. BF2
  `FUN_0030c8f8` picks one of up to five `Effect` rows at random (AGENT_C 13).
- [V] **MuzzleFlash is not an effect** (B1 `FUN_002864f0` 210374–210409): the value selects `small_muzzle_flash` /
  `med_muzzle_flash` / `large_muzzle_flash` and writes hardcoded weapon fields — +0x1a4 = 0.6 / 1.0 / 1.8, +0x1a8 =
  0.15 / 0.25 / 0.45, +0x194 = 0.1 / 0.15 / 0.3, +0x1ac = 0xff32ffff, +0x1b0 = 0xff0d3f3f / 0xff197f7f / 0xff25bfbf,
  +0x1b8 = 0 and +0x1b4 = 0; any other value keeps the ctor's. The particle-effect keys on weapons are
  `MuzzleFlashEffect` (fusion cutter) and `ChargeUpEffect`. `Discharge` and `ExpireEffect` are read by neither ELF
  (elfword: 0).
- [V] Others (AGENT_C §2–§3, setters cited there): `TrailEffect` (ordnance, attached at launch), `DamageEffect`
  (vehicle damage stages), `ChunkTrailEffect` / `ChunkSmokeEffect` / `ChunkTerrainEffect` (explosion debris),
  `ThrustEffect`, `ContrailEffect` (BF2), `WakeEffect`, `WaterEffect`, `StompEffect`, `DeathEffect`, `JetEffect`,
  soldier `WaterSplashEffect` / `FootWaterSplashEffect` / `WakeWaterSplashEffect`, props `AttachEffect` / `Emitter`, the
  hardcoded `flyerspray`, `headshot`, the dying building's `explosion` / `med_explosion` / `small_explosion`.
- [D] **Showcase census** (AGENT_C §4, `AGENT_C_weapons.txt`): every infantry bolt on BF1 NAB1 / TAT1 / YAV2 impacts
  as `ord_explosion` (all five surfaces) and `watersplash_ord`; BF2 UTA1 / MYG1 / HOT1 as `com_sfx_ord_exp` and
  `com_sfx_watersplash_ord`; grenades and launchers use per-weapon `Effect`s (`com_sfx_weap_grenade_thermal_exp`,
  `com_sfx_weap_rad_exp_md`, …) with `TrailEffect`s; nearly all live in COMMON / INGAME. BF2 names eight BF1-only effects
  (e.g. 178 `ChunkTerrainEffect=dirtspray` rows) that no BF2 LVL holds — the PS2 draws nothing there, so neither should
  the port.

## 8. BF2 — the twin

Read by a helper at the BF2 twins of every §5–§6 function (`AGENT_D_bf2twin.md`; the lead re-read the starred lines).
BF2's emitter update is `FUN_00375210` (the EMITTER test at B2 371917), spawn `FUN_00379518` / `FUN_00379748`, chains
`FUN_0037ac88` / `FUN_0037af20` / `FUN_0037b0c0` / `FUN_0037b198`, instance init `FUN_00374570`, retire `FUN_00374de0`,
per-particle draw `FUN_00377cc0`, cull/fade `FUN_0037a090`, state records `FUN_005957a8`.
- [V] **The same:** the stateless re-spawn (372101, 372208); bursts, rounding and the lifetime budget (retire 371806);
  127 per frame (372021); STREAK < 3 (372260); the continuous-emitter rule; shapes, spawn and inheritance (374702–374768);
  FadeInTime on the emitter clock (372121–372130); every chain op incl. the Move quirk (`FUN_0037af20` 375645, asm) and
  the colour clamp; the emitter colour multiply (372245); MinLodDist / MaxLodDist unread; far distances 50 / 150 / 500 m
  (`FUN_001b4ef0` / `4f28` / `4f60`, 59872–59912) with the 20 % fade; the thinning tables; sizeMul = 1 + 0.2·clamp
  (375046); the PARTICLE sprite (same packing 727046–727047, and a VU1 overlay identical to BF1's but for 13 address
  immediates); BILLBOARD's plane table; SPARK / STREAK; GEOMETRY's matrix; the GS equations — records 0x7ca810…0x7ca860
  (`FUN_005957a8` 727710–727733): ALPHA 0x100044 / 0x100048 / 0x100040 / 0x100154, TEST 0x51001, PRIM 0x58 (no fog,
  no depth write); colour halving.
- [V][O] **Different — no 20-variant cache\*.** Each particle gets its own seed, `((idx·0x40018fc9 + r·0x4000029d) %
  0x40000003) ^ 0xc71c2830` (`FUN_00379518` 374664), and is sampled straight into floats (`FUN_00379748`): no 16-bit
  quantisation, so none of BF1's rotation / size wraps.
- [V][O] **Different — caps\*.** MaxParticles is clamped to 128 per instance (`FUN_00374570` 371576; −1 untouched) and
  a burst to 128 (372041). BurstDelay's pair is multiplied by the viewport count n at parse, unconditionally (×1 in
  single player; 376088–376095, asm), and MaxParticles is halved when n ≥ 2 (§3) — the split-screen budget.
- [V][O] **Different — ANIMATED.** Quads are built on the CPU (`FUN_00594990`): half-width size′ (**2·size′ wide**, not
  √2·size′), angle rot·2π/512 (0.703° per unit, no wrap). Frame\* (372212–372220): f = (int)(age/TimePerFrame); with
  `Looping` (the default) f %= TotalFrames, without it f ≥ TotalFrames is not drawn. UV (`FUN_00377cc0` 374247–):
  a grid of 2^(log2 W − log2 FrameSize) columns × 2^(log2 H − log2 FrameSize) rows, row-major — `FrameSize` is the cell
  edge in texels (256² with FrameSize 64 = 4×4; BF2's 7 ANIMATED emitters all make 4×4 grids).
- [V] **Different — small:** BILLBOARD's angle is continuous (an interpolated 2048-entry table) and, with
  NoRegisterStep, its axes follow the emitter node; GEOMETRY's ADDITIVE model flag is 0x800001 (BF1 0x400001); the
  per-particle box is ±450 m (BF1 ±200 m); an attached effect's live particles follow its translation (`FUN_00377620`
  373596–373614, the gate [H]); BlurRes is stored but never read.
- [V] **BF2-only:** the Lua `SetParticleLODBias(x)` adds x to the three far distances (`FUN_0037a040`, 374962) — POL1
  and the space maps call it, **UTA1 / MYG1 / HOT1 do not** [D: the .luac constant pools]. With the world HDR effect on
  (`DAT_007c5754` = 0.5, set by `FUN_00456098` 516764 through the setter `FUN_0057fd40`; BSS 0 at load, 1.0 from
  `FUN_00580c80` at init — its only other writer), particle RGB is halved a second time (726872, 726940); forced back to
  1.0 in split-screen. Which maps enable HDR is [H] (a FXP2 check: grep the map `fx__` for `Effect("HDR")`).

## 9. PS2 frames

- [V] `lead_tmp/fxp/ps2cap/` (this session; NAB1 and HOT1 spawn states, fire held 4 s pitched down, 30 fps
  recordings + stills): `crop_nab1_impact.png` — BF1 `ord_explosion` on grass: a white flash, then small yellow-orange
  additive sparks (the `a_sparks3` flipbook) inside ~20 px at ~15 m; the red bolt heads glow as soft red blobs.
  `sheet_hot1_fire.png` — BF2 HOT1: the impact flash is small; the big pink wash over the snow is the bolt's light,
  not a particle. These are the "before" references for FXP2/FXP3's pairs; the muzzle flash was not framed (the
  player's own body hides it in the third-person view).

## 10. What the port needs — the cards

- **FXP1** (build, M1, alpha): `mungefs::fx` parses `ParticleEmitter` trees (every §2 key, both layouts' semantics,
  §3's rules, defaults) keyed by the NAME hash; the bake writes the effects of COMMON / INGAME, the side LVLs and each
  map (texture names already baked). Gate: a data test reproduces §1's counts (128/701, 292/1702) and §4's RAM field
  values for a sample of templates.
- **FXP2** (build, deps FXP1): `game::fx` runs emitters per §5 (bursts, the budget, the 20 variants, spawn, the
  Transformer closed forms, emitter colour, fade-in) and draws §6: PARTICLE / ANIMATED squares (√2·size′, the rotation
  mapping, the atlas), BILLBOARD planes, SPARK / STREAK ribbons, GEOMETRY models, EMITTER children; ADDITIVE and NORMAL
  blends with depth test and no depth write, no fog, PARTICLE sorted back-to-front; the §6.8 distance fade and
  density thinning; BLUR as a PROVISIONAL plain NORMAL sprite until its pass is read. Removes the placeholder quads
  and fx.rs's stale HOLD note.
- **FXP3** (build, deps FXP2): the spawn hooks — sim events carry the ordnance class and the struck surface kind; bolt
  impacts pick §7's slot; explosions spawn `Effect` / `WaterEffect` (the 60 m rule is a PS2 budget — modern: keep all);
  TrailEffect on rounds; vehicle DamageEffect / Chunk* / Thrust / Wake / Stomp. Pairs: the §9 impacts + an explosion.
- **FXMF** (re → build): the muzzle-flash presets' renderer (what +0x194 / +0x1a4 / +0x1a8 / +0x1ac / +0x1b0 draw).
- **FXB** (re → build): the laser bolt's own draw (ordnance LaserTexture / glow / light — the pink wash in §9).

## 11. Still [H]

- BLUR's second pass (the strip microprogram, how BlurValue offsets the screen copy); GEOMETRY flag 0x41 (BF2
  0x800001's 0x800000 too); the near-scene fade's inputs (DAT_00677660/664, [H] fog range); where the 2000-sprite
  counter resets; how buckets and effects order against each other; what the fallback texture shows.
- Vortex / type-4 shapes (3 BF1 HOT1 emitters); the emitter-path node spacing (~0.1 s, |dv|² > 0.5) as it affects
  moving emitters (AGENT_A 0.3).
- The names of Spawner type 4 and its two X/Y/Z families.

## 12. Checks run (§5 of the prompt) — `checks.txt` holds the full output

`bash engine/artifacts/lead_tmp/fxp/checks.sh` (output `checks.txt`, 291 lines), in short:
1. **fn** — 88 B1 and 18 B2 cited lines, every one in the function this document names (the retire lines 241940 /
   241982 in `FUN_002b26a0`, the STREAK side vector 543776 in `FUN_0047df68`, the fallback texture 563013–563017 in
   `FUN_0049ee68`; B2 69002/69009/66186 are the viewport-count uses in `FUN_001c1260` / `FUN_001bd800`).
2. **absent** — MinLodDist / MaxLodDist: every B1 +0x200 / +0x204 hit in 240000–250000 is a write (241575–241576 and
   241617–241618 the ctors, 247092 / 247102 the parser); B2 +0x194 / +0x198 in 370000–378000 likewise (371419–371420,
   376161, 376286). FrameSize: the only B1 +0x1e4 hits (249129–249442) are in `FUN_002bd8c8` … `FUN_002be1a0`, another
   object. `Looping` (0x7593992d) in B1: `FUN_003572e8` and `FUN_0046bc28` only (not the particle parser).
   `SetParticleLODBias`: only BF2 MISSION.LVL's pol1*/spa* scripts carry the string.
3. **twin** — §8 (the helper's per-claim table in `AGENT_D_bf2twin.md`, lines re-run through `fn b2`).
4. **word** — 'WhiteSmoke' @0x53f480; … the keep table 0x518408 = {1111, 1110,
   1010, 1000}, the fade table 0x518448 = {0001, 0100, 0010, 0000}; 0.00314 = `lui 0x3b4d / ori 0xc875` at
   0x4b026c/0x4b0270; B2 0x5fea68 = 16.0; the GS records' immediates (`ori a1, a1, 0x4044 / 0x44 / 0x4048 / 0x48 / 0x40 /
   0x154`, `ori v0, v0, 0x1001` after `lui 0x5`, `addiu a3, zero, 88`); the ctor's `lui at, 0x4120 / 0x4248 / 0x40a0`;
   *(decompiler detail omitted)* = 0x42480000 / 0x43160000 / 0x43fa0000.
5. **writers** — DAT_00518494, DAT_00518490 (B1) and DAT_005fea68 (B2): no writes.
- **RAM** — `ramcheck1.py` + `ramcheck2.py` (BF1 NAB1: 640 templates, 608 matched; timing / LOD 608/608, 16 more field
  groups with 2 disagreements on one sibling match) and `ramcheck_bf2.py` (BF2 HOT1: 856 templates, 23 field groups,
  0 disagreements).
- **Verifier** (one foreground Opus agent, claims + citations only, `verify/CLAIMS.md` → `verify/VERDICTS.md`):
  **34 SUPPORTED, 6 PARTLY, 0 NOT SUPPORTED**, all six PARTLY folded in above (C8 the RAM-script attribution — it then
  extended the check to every field, 608/608; C11 the decode lines; C20 the view's +0x90 zoom factor; C28 type 8
  draws no decal; C30 the +0x1b4 write; C35 the B2 record lines). It decoded the VU1 overlay end to end (C22) and found
  no reader for any absence claim (C7, C21, C27, C40) at the instruction level.
- **callers** — the parser `FUN_002ba2e8` is reached only from the chunk loader wrapper `FUN_002ba1d0` and itself (the
  Geometry EMITTER child); the spawn API `FUN_0036d478` has 19 callers.

## 12. Gain-effect child rendering follow-up (2026-10-26) [O] UNAUDITED

The BF1 BES1 resupply comparison exposed a runtime gap rather than a missing bake: `Powerup_red` and `Powerup_blue` contain EMITTER geometry children which the old particle draw path skipped. Their BF2 counterparts use the same hierarchy. `game/src/fx/children.rs` now replays child templates along their parent particles in the native 0.3 s steps and submits their particles with the existing draw implementation; definitions and variant caches are prepared once. Gain effects follow the receiving unit and deduplicate overlapping gains while a matching effect is alive. Full measurement, source checks, regression failures, GPU captures and limits are in `re-powerup-station.md` section 15. This does not implement GEOMETRY-model particles or claim generic nested effects are pixel-identical.

The maintainer's recorded comparison prompted a further correction (section 16 of `re-powerup-station.md`): the first child draw path grouped several authored bursts onto each 0.3 s sample. Child births now interpolate along those path segments at their own burst times. Ordinary emitters retain their existing constant-frame update path. The spacing regression fails on the first implementation's three-height result, and passes after the correction; a scheduling regression separately holds birth times to their positions.

## 13. EMITTER child replay, full read (2026-10-26) [V][O] UNAUDITED

Re-read of the native child path, both games, against `game/src/fx/children.rs`. B1 line numbers; the BF2 twin `FUN_00375c98` (360021–360429) has the same structure: 31-index cap (360206), 0.3 s step (360290), child-mode flag `DAT_005fef3c` around the update (360346–360350), tint through `FUN_00376d70` → `FUN_00374d60`, which replaces the instance colour like B1's `FUN_002b2620`.

- **Gate.** `FUN_002b2b10` takes the EMITTER branch only while `DAT_005183f8 != 0` (241876). ELF word 0x5183f8 = 1, and no text write exists (`re_check.py writers b1 DAT_005183f8` → 0), so the original draws children.
- **Parents.** The burst walk is the ordinary one, but the loop stops at particle index > 30 (*(decompiler detail omitted)*, and *(decompiler detail omitted)* per particle). This is an absolute spawn index, not a count of live particles. There is no `FUN_002b8a30` density thinning and no FadeInTime multiply on parents.
- **Scratch init** (`FUN_002b45f8` 243024): one static effect at 0x58d420. Per parent, its instance offset = parent offset + idx·10 (*(decompiler detail omitted)* + idx·10), and each chained sibling instance +5 (`FUN_002b5f78`). The child template's seed word +0x78 is set to a draw from the parent's per-burst LCG chain (`FUN_002b4258`). Time +0xfc, node timer +0xf8 and node list +0xf4 are reset.
- **Replay.** Samples at parent ages 0, 0.3, 0.6, …, clamped to the parent's age a. At each sample the scratch matrix is identity rows (`DAT_0066ef20`) with the parent particle's position at that age (`FUN_0036d478`). The scratch is updated by dt = min(step, sample) (`FUN_002b4df0`), with the first dt = 0.
- **Path nodes** (`FUN_002b4858`, child mode forced by `DAT_005183fc = 1`):
  - The first update makes node 0 at time 0 with zero velocity (`DAT_0066eee0`, zeroed by `FUN_00445fa8`).
  - On each later update the timer drops by dt. If there is no older node, or the timer is below 0, a new node is made at (now, position), the previous node's velocity becomes the secant (P_new − P_prev)/(now − T_prev), its span is set to that interval, and the timer resets to 0.1 s.
  - Otherwise (a step of 0.1 s or less after a new node) the newest node is moved to (now, position), the older node's span grows by the step, and its velocity becomes (P − P_older)/(span + 0.0001). A last step of 0.1 s or less therefore drops the previous 0.3 s sample.
  - The newest node keeps zero velocity.
- **Births** use the ordinary stateless walk over these nodes: P_k + v_k·(b − T_k). Within a segment that is linear interpolation between the kept samples.
- **Tint** (`FUN_002b4778` → `FUN_002b2620`, after the replay): every child instance's emitter-level RGBA is **replaced** by the parent particle's colour at age a × the parent instance's RGBA. The values are raw: an HSV parent passes H, S, V as multipliers. Then the scratch is drawn.
- **Live check, PCSX2** [M] (BF1 BES1, read-only PINE reads of the scratch at 0x58d420 while the maintainer threw a grenade and stood at a health and an ammo droid): 19 native replays were captured, 15 with complete node lists, at parent ages 0.033–0.968 s. Archive: `engine/artifacts/lead_tmp/resupply-native-child-nodes.json`.
  - Every node list and timer follows the rule above. At ages 0.0334–0.2669 there are two nodes. At 0.3003, 0.3337 and 0.3670 the 0.3 s sample is merged, with timers 0.0997, 0.0663 and 0.0330. At 0.4004 a step of 0.1004 s adds a third node. At exactly 0.3000 there are two nodes, and the timers at 0.634 and 0.968 imply merged steps of 0.034 and 0.068.
  - Velocities: a node that gets a successor has velocity equal to the secant within 4e-7. A merged node's velocity equals secant/(span + 0.0001) within 3e-7. The newest node's velocity is zero.
  - An earlier stale read held time 3.970612 s with timer 0.029388, which is also a merged step.
  - `fx::tests::resupply::the_replay_nodes_match_live_bf1_captures` runs the port's `path` on three of these captures (two nodes, merged, three nodes) and checks their node times and velocities.

Port (`children.rs`): `path` and `frame_at` implement the sampling, node and merge rules exactly, in f32 and in the native order. Parents are evaluated per index at an exact age (`Instance::particle_at`), so rounding in `t − age` cannot drop a parent. Parents are unthinned with index < 31. Child offsets follow the native rule. Child instance RGBA is replaced by the parent's raw RGBA. The node matrices are identity.

Not matched:
- **Seeds.** The child's burst seed comes from the native parent LCG chain, which the port's burst RNG does not reproduce, and that was already true for every effect. The child uses a deterministic visual seed instead.
- **BF1 child variant caches** come from the effect hash, not the global construction order.
- **The outer per-effect fade gate** in `LiveEffect::draws` uses the parent emitter's BoundingRadius at the effect origin, and the native caller's gate for EMITTER was not traced.

## 14. Water impacts, water blasts and ordnance trails (2026-10-26) [V][O][M] UNAUDITED

BF1 KAS1 (`bf1_kas1`) showed three effects missing from the port: no splash where a shot hit the water, the land fireball for a blast in the water, and no red glow on a thrown thermal detonator. B1 line numbers below; check each one with `python scripts/re_check.py fn b1 <line>`.

**Live state, PCSX2** [M]. SLUS-20898, paused, read-only PINE reads (port 28012), no writes and no state load. Scripts: `engine/artifacts/lead_tmp/water-native*.py`. Output: `water-native-pool.json`, `water-native-live.json`, `water-native-trail.json`.
- Water: level `DAT_00564908` = −45.0, bob `DAT_005229cc` = 0.02685, a 64 × 64 wet grid of 32 m cells (`DAT_005648dc` nibble map, offset 1024). The water is flat (`DAT_005647b8` = 0).
- The splash pool `DAT_00519508` (0x112a9a0) held 5 records. Every record's y = −44.95315, which is the surface with bob + 0.02, and every one sat in a wet cell.
  - Four were `watersplash_ord` (bolt hits), at ages 0.10–0.67 s.
  - One was `watersplash_lg` at (210.79, −38.07): a thermal detonator's `WaterEffect`. No land explosion effect was live near it.
- A thrown thermal detonator (sticky, vtable 0x53aa18, at 0xd34250) carried `com_sfx_weap_grenade_red` (instance 0x1cf8a10).
  - The effect's position equalled the round's +0x40 exactly.
  - Its rotation rows were x = normalize(up × dir), y = dir × x, z = dir, along the launch direction.
  - Class +0x70 held the trail definition. Class +0xd4 = 0xff272794 (`LightColor` premultiplied) and +0xd8 = 4.0 (`LightRadius`).
- Quirk: two `com_sfx_weap_grenade_blue` instances were bound to freed memory that now held laser bolts (vtable 0x53a568). A stopped trail keeps its stale position pointer. The port does not copy this.

**Water as a collision layer** [V]. `FUN_00220cb8` creates the water object with layer 0x80 and registers it through `FUN_00161b80` (`callers b1 FUN_00161b80` → B1:135021). Its vtable is 0x533e30 (`word b1 0x533e30 12 --u32`), and +0x20 = `FUN_00220ef8` → `FUN_00476c10`.
- `FUN_00476c10` (538878) computes h = level + bob − origin.y and t = h / dir.y. It hits only when h ≤ 0, t ≥ 0, and the cell is wet (`FUN_00477458`). The normal is (0, 1, 0).
- The bolt's per-frame sweep `FUN_00273128` uses mask 0x9de, or 0x996 after the predictive ray. Both masks include 0x80, so a round stops at the water.

**Bolt impact on water** [V]. `FUN_00276810`, class 0x80 (B1:197741):
1. `FUN_00478478(1.5, 0.01, pos)` makes a ripple.
2. `FUN_0037c920(class+0xc0 = ImpactEffectWater, pos)` spawns the splash.
3. The generic effect is skipped.
4. The base `FUN_00272d70` then plays the CollisionSound and the impact explosion.

`FUN_0037c920` → `FUN_0037cdc0` (369209) spawns into a 200-record pool with world axes at pos.y + 0.02. `FUN_0037cc98` (369147) re-snaps each record's y to the surface (with bob) + 0.02 every frame.

**Explosion in water** [V]. `FUN_002701a0` (193432):
- `FUN_00354fe0(+0x9c)` always plays the `SoundProperty` first.
- The wet test `FUN_004768f0` then sets in-water = pos.y ≤ h + 0.3 (193540).
- Within 60 m and in water, with `DamageRadius` / `DamageRadiusOuter` (+0x58, one field) > 0, it runs:
  - a ripple `FUN_00478478`;
  - `…
  - `FUN_001a8740` (the spray pool, gated by `DAT_00517464`, which is 0 live).
- In that branch there is no land `Effect` and no decal, light or effect sound (+0x90's sound plays only beyond 60 m, 193554).
- With +0x58 ≤ 0 in water, nothing visual is spawned.
- `writers b1 DAT_00517464`: set only by `FUN_001a8338` / `FUN_001a83c8`.

**Trail** [V]. The ordnance ctor `FUN_002728a8` instantiates class+0x70 (`TrailEffect`, setter `FUN_002738a8`, `hash TrailEffect` → B1:195637).
- It binds the effect to the round's +0x40 with `FUN_0036d478(e, obj+0x40, 1)` (194956) and orients it with `FUN_0036d590(e, launch vel)` (194966).
- `FUN_00272c28` (195004) stops it with the round.
- `LightColor` / `LightRadius` (`hash LightColor` → the bullet setter `FUN_002774b0` 198178) are stored, but BF1 never reads +0xd0..+0xdb (re-bolt §5.1). BF1's grenade glow is therefore the trail effect: `com_sfx_weap_grenade_red` is one additive red `lightflare` emitter with `NoRegisterStep`, so it rides the round.

**Port.**
- `core::projectile::Bolt::spawn` takes the water plane as a fourth candidate: crossed downward, nearest wins, `Hit::Water`, hash tag 6. `Sim::resolve_bolt` sends `Surface::Water`.
- `game/src/fx.rs`:
  - spawns `ImpactEffectWater` at (x, water + 0.02, z);
  - `Fx::explode` applies the native rule, using `ExplosionLook::radius_outer` from `DamageRadius` / `DamageRadiusOuter`;
  - the impact explosion of a water hit runs through the same rule.
- `game/src/fx/trail.rs` starts every ordnance's `TrailEffect`, bolts and thrown grenades alike, at the round with the measured axes, carries it each tick, and stops it when the round is gone. It is render-side only, so the sim is unchanged apart from `Hit::Water`.
- Regressions:
  - `core sim::combat::tests::water_stops_bolts_and_names_the_surface`;
  - `hash_bolts` (Water is not an alias);
  - `fx::tests::water_takes_the_impact_onto_its_surface_and_swaps_the_blast`, `a_round_carries_its_trail_until_it_is_gone`, `grenade_trails_keep_their_rounds_when_an_earlier_one_goes_off`;
  - the asset check `game/tests/suite/water_fx.rs`: the real BF1 class db and effects, the red glow on the GPU, and `watersplash_lg` with no land effect when the fuse runs out in the water. `OPENBF_WATER_CAPTURE=<dir>` writes the frame.

Not matched (the wet mask, the ripples and the splash re-snap were added later, §15):
- The `FUN_001a8740` spray. It is gated off in the measured session.
- The 200-splash cap and the 60 m effect gate (dropped as before).
- BF2's own water rule and BF2's sticky light: not read.
- The stale-trail quirk.

## 15. Ripples, the splash bob and the WMAP wet mask (2026-10-26) [V][M] UNAUDITED

B1 line numbers; check each with `python scripts/re_check.py fn b1 <line>`.

**Live state, PCSX2** [M]. The same paused SLUS-20898 session, read-only PINE (no unpause, no state load, no RAM write). Script `engine/artifacts/lead_tmp/water-ripples.py`, output `water-ripples.json`.
- Ripple count `DAT_005229c8` = 5, clock 30.246 s.
- Four bolt ripples: life 1.5, size 0.01, y = −45.0 (the level, no bob), kind 0, seen 1.
- One explosion ripple at (210.79, −45, −38.07): life 3.4, size 0.4, beside the `watersplash_lg` of §14.
- Slope factor `DAT_00522a84` = 2.0 (`word b1 0x522a84 1 --f32`).
- Water colour `DAT_0069c348` = (128, 130, 128, 80), the KAS1 manifest's `max_diffuse`. Ambient `DAT_0069c570` = (90, 90, 100), sun `DAT_0069c588` = (120, 120, 130), light direction `DAT_0069c590` = (8.7e-8, −1, 0). These are the manifest's `ambient_sky` and `sun_color` back in sRGB bytes, and its `sun_dir`.

**Creator `FUN_00478478(life, size, pos)`** [V] (539767).
- Only on flat water (`DAT_005647b8` == 0), only while the count is below 100, and only over a wet cell. Otherwise nothing.
- Record (0x30 bytes at `DAT_00676230`): pos (x, level, z), velocity, end = now + life, start = now, size, kind byte (0 static), seen byte = 1.
- `callers b1 FUN_00478478`:
  - 197740 `FUN_00276810`, bolt impact on water: (1.5, 0.01);
  - 193560 `FUN_002701a0`, explosion in water: asm 0x2702ec–0x270314 gives s = min(0.4, R / 2) with R = +0x58, and life = 3.0 + s in the delay slot;
  - 201010 `FUN_0027b068`, the grenade base on water contact: (1.5, 0.01);
  - 127995 `FUN_00216b30`: (3.0, 0.5).

**Update `FUN_004774f0`** [V] (539136), from the water tick `FUN_0048b1e0`. A record whose seen byte is 0, or whose end has passed, is replaced by the last one (the count drops and the same slot is tested again). Otherwise seen is cleared.

**Draw `FUN_0048d2b8`** [V] (551751).
- Seen is set when the camera is within 120 m (distance² < 14400), before the frustum test.
- Radius r = size + 1.5 · age. Centre y = level `DAT_00564908` + bob `DAT_005229cc`.
- Texture `DAT_0069c36c` = `FXrippl1` (`word b1 0x558e48 8 --str`). Blend NORMAL (ALPHA 0x44), depth test on, no depth write, no alpha test.

**Ring `FUN_0048d990`** [V] (551951).
- a = age / life; q = a · 6.6666665 below 0.15, else (1 − a) · 1.1764705; nothing below 0.
- p = min(1, q · (1/3) · life); tilt A = p (2 − p) · 0.3.
- 21 columns at j · 0.31415927 rad; radii r − 0.375, r − 0.1275, r + 0.1275, r + 0.375, at y + 0.01.
- Normal n = normalize(2A cos, 0.5025, 2A sin). Inner shade d = max(0, n.x L.x + n.z L.z − n.y L.y); outer d = max(0, −n · L).
- Colour = W (Amb + S d) + S d⁴ per channel, bytes (int)(c · 255), alpha (int)(p · 255). The two edge rows are round(W · 128) on the GS scale with alpha 0.
- U = j · 0.5; V = 0, 675/2048, 1372/2048, 1.

**Bob** [V]. `FUN_00475658` (538091) sets `DAT_005229cc` = 0.06 sin(1.8849558 t) on flat water. `FUN_0037cc98` re-snaps each splash to level + bob + 0.02 every frame (§14).

**Wet mask** [V]. `FUN_00477458` (539120): a nibble per cell, row-major [z][x], high nibble for even x, non-zero is wet. The cell is clamp(int((v + half) / cell), 0, grid − 1), with NaN or negative giving 0. `FUN_00476c10` tests the crossing point's cell; `FUN_004768f0` (538777) is the point query. KAS1: 64 × 64 cells of 32 m, 4014 wet.

**BF2** [V, not measured]. Creator `FUN_00562f18` with the same callers and arguments; the ring constants are the same.

**Port.**
- `core/src/water.rs` `WetMask` and `Plane`. `Sim::water_mask` (a load-time input, not hashed) comes from the manifest's new `water.wet` (bake: `mungefs` WMAP). Bolts, grenade sweeps, the AI's water checks and foley all use it.
- `game/src/fx/ripple.rs`: the pool, update and ring as above, spawned by bolt water hits and water blasts on non-ocean water. The ring is a `ParticleShape::Shaded` draw.
- Water splashes (`ImpactEffectWater`, `WaterEffect`) are re-snapped each frame to level + `flat_lift` + 0.02.
- Regressions: `core water::tests`, `sim::combat::tests::water_stops_bolts_and_names_the_surface` (dry cell), `game fx::ripple::tests`, `fx::tests::water_hits_leave_ripples_and_their_splashes_ride_the_bob`, `render particles::tests::shaded_triangles_carry_their_own_vertex_bytes`, and `game/tests/suite/water_fx.rs` `kas1_carries_its_wmap_mask_and_a_ripple_reaches_the_frame` (4014 wet cells, a ripple on the GPU).

Not matched:
- (Matched since issue #70: the sim's water plane bobs by the PS2's ±6 cm, for bolts, grenades, droidekas and soldiers; `FUN_00476c10` tests the crossing at level + bob. The port's own bot rules stay at the layer height.)
- Wader ripples (kind 1, `FUN_00478778`), `FUN_00216b30` and the grenade-base ripple (the port has no grenade base).
- The frustum test (the port draws every ripple in the pool) and fog on the ring.
- BF2 was not measured.

## 16. BLUR, the blend space and GEOMETRY segment looks (2026-10-29) [V][O] UNAUDITED

- [O] **BLUR writes no particle colour** (B1 records 571949–571961, B2 715450–715452): pass 1's ALPHA byte 0x40 is
  A Cs, B Cs, C As, D Cd = **Cd** (only the particle's alpha reaches the frame's alpha plane); pass 2's 0x54 =
  (Cs − Cd)·Ad + Cd with Cs a screen copy, laid in through that alpha. The texture's colour never shows. The port draws
  nothing for BLUR (MUS1's `mus_sfx_blur` heat rows were white sheets as a NORMAL sprite); the haze of pass 2 stays §11 [H].
- [D] **Particles blend in display bytes** (§6.7's ALPHA 0x44 / 0x48 act on the GS's stored bytes). Drawn on the sRGB
  target they blended in linear light: a faint fringe (BF2 `underlitesmoke2`, a 16-colour sheet whose alphas run 0, 15,
  32, 48, 59, 100…) nearly vanished and the body appeared at once — GEO1's smoke column looked thin and cut. Bolts and
  particles now draw in their own pass through the frame's non-sRGB view, the texel re-encoded before the vertex colour
  modulates it (`render::bolt::DisplayPipelines`; GPU suite `particle_blend`). Filtering stays in linear light.
- [O] **GEOMETRY draw flags, BF2** (`FUN_00377cc0` 361960–361968): the model is drawn with F = 1 for NORMAL, 0x800001 for
  ADDITIVE, 0x41 for BLUR, colour = the particle's bytes. **Segment look** (B1 `RedModel::Draw` 557304–557311 and the
  normal class 594259–594365; B2 `FUN_0058ed70`, `analysis/re-material-flags.md`): translucent when the emitter is
  ADDITIVE, the tint's alpha is not 255, or the segment blends (0x04 without 0x02); a translucent segment is additive
  when the emitter is ADDITIVE or the MTRL additive bit is set (B1 0x80, B2 0x40) and takes the particle's colour; an
  opaque one is white, unblended, depth written. Port: `fx::draw::geometry_look`, `ParticleShape::SolidMesh`.
  PROVISIONAL: an alpha-tested (0x02) segment is drawn blended by its sheet rather than tested at AREF 0x60.
- [V] **Effect model sheets keep their disc alpha.** BF2 END1's bunker class runs `Emitter = "bunker_lightning
  hp_lightning"`: 14 GEOMETRY emitters drawing `bunker_lightning1..4` (segment 0x10000) whose tint flashes 0 → 0.996 →
  0, so the segment is always translucent. Its 32-bpp sheet is red with **alpha 0** behind white bolts (disc texels
  `[00,00,00,a7]`), but the map bake dropped alpha from any sheet no MASK/BLEND segment names, so it drew a red sheet.
  The map bake now keeps the alpha of every texture a GEOMETRY model names (`fx_model_named`), and the side/common
  effect bake keeps it for all its model sheets.
