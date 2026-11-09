# OPUS RE (2026-09-28) - [O] UNAUDITED
# RE: a vehicle's death — the blast's area damage and push, and the chunk bodies (card VDTH)

> **AUDIT 2026-09-29 (loop lane 1, PARTLY — `engine/artifacts/loop/audits.md`).** Re-derived from the B1/B2 decomp,
> the ELF instructions, the disc, PS2 RAM and a live PCSX2 run, before reading this doc. Everything the VDTH build
> uses holds:
> - the gather (box half-side, cap 20, masks 0x81e / 0x103b) and the LOS masks (B1 0x40 = terrain at world+0xbc,
>   0x8 = the building layer; B2 0x80 at world+0xd0, 0x10);
> - the ($f12 Inner, $f13 Outer) registers and the falloff, with B2's class+0x37c factor;
> - the setter offsets and the constructor defaults;
> - w, reproduced (STAP springs 4.0043, AAT springs 6.3446, and the stance radii);
> - the rider rule (the mount's layer is 8 / 0x10);
> - TakeDamage and the vehicle's own blast (manned: +0x380 = the driver's controller);
> - every chunk-launch rule and the 3 / 8 / 5 s rest limits (BF1's vehicle chunks are all ChunkPhysics FULL; no BF1
>   class authors ChunkLifeTime 0x4f17fc11).
>
> **Corrections**, all verified by one Opus agent (9 of 9 SUPPORTED; `lead_tmp/audit-vdth/verify/VERDICTS.md`):
> 1. **BF1 pushes corpses too, not "the living".** FUN_002009a0 skips only soldier states 5, 8, 9, 12 and 13. Death
>    sets state 3 (FUN_001ffec0 via the health interface's +0xc). B1 has no PushDeadOnly (hash absent from the ELF).
>    Corpses stay in the gathered soldier layer.
> 2. **§5's "fling … death animation / ragdoll [H]" is a velocity kick.** After the splash, a soldier who is dead gets
>    FUN_00200f90 (B2 FUN_002395a8): on foot, +0xa20 += dir × 5.0 (B1 DAT_00517878, ELF 5.0, no writer; B2 literal).
>    A mounted soldier (state 12/13) is dismounted and thrown at (d.x, d.y + U·0.3 + 0.4, d.z) × 9. The kick also
>    lands on corpses killed earlier.
>    - Live on the PS2: a corpse at rest for 1.87 s got exactly 5.00 m/s when a blast killed a soldier 5.5 m away
>      (`lead_tmp/audit-vdth/corpse_samples.csv`). A thrown rider flew 22.3 m, because corpses integrate their
>      velocity.
>    - A state-3 corpse fades between 3 and 4 s after death and is not drawn after 4 s (FUN_002013a0).
> 3. **§5's "resist" roll is live.** soldier+0x290 is the class, and class+0x1e58 is **ChunkFrequency** (0x43b50014).
>    Every BF1 droid-soldier class sets it to 0.6 (disc cis_inf_battledroid; RAM on KAS2, GEO1, NAB2). A roll ≤ 0.6
>    skips the push and sets soldier+0xbfd bit 3, which the death tick reads to blow the droid apart
>    (ExplosionName + chunks, B1:116889-116968).
> 4. **§4's [H]** — an empty BF1 hover has +0x380 = 0 and team 0 (RAM).
>
> Scripts: `engine/artifacts/lead_tmp/audit-vdth/`. The corrections go to card KNOCK.
>
> **CORRECTION 2026-10-07 (card KNOCK, `analysis/re-knockback.md`).** §5 / §7's "no disc class authors PushDeadOnly 0"
> is wrong: `expc.py` reads the baked class data, which holds neither the EWK nor the SNW side. A raw scan of the BF2
> disc finds `ewk_weap_inf_rock_exp` (Push 4, PushRadius 0.2 - 0.3) and `hoth_weap_inf_smash_exp` (Push 8, 3.5 - 4.5)
> authoring 0: the Ewok's rock and the wampa's smash push the living. What a pushed soldier does (the five skipped
> states are thrown / tumble / get-up / the two mounted ones), the death kick's direct-hit source, the late death and
> the droid chunk death are read and measured live in `re-knockback.md`.

**The question** (card VDTH, re-vehicles.md §6): when a hull pool runs out the PS2 detonates the class's `_exp`
class and throws the `CHUNKn` pieces in the same tick. What does the blast do to what stands near it (the
multiplier's shape, inner/outer radius, LOS or not, team rules, the victims' per-type scales), what pushes, and how do
the chunks fly, bounce and expire — in both games?
**Method.** Read the explosion class (ctor, setter, the one explode function every explosion source calls), its
per-victim virtuals, the damage application, the chunk launch and the chunk body, in B1 and B2; ran ELF word / vtable
scans for the virtual slots and constants; cracked every load-bearing property hash by FNV (`fnv(name) == hash`); read
the victims' collision spheres out of PS2 RAM (`engine/artifacts/pcsx2/ref/states/bf1_*.p2s`, EE RAM) and the
`modl SPHR` chunks off the disc. Tools in `engine/artifacts/lead_tmp/re/vdth/`: `expc.py` (the discs' `expc`/`entc`
classes with named props), `setter2.py`, `vt.py` (vtables by slot), `sphere_ram.py` / `soldier_w.py` / `veh_w.py`
(RAM), `modlinfo.py`, `census.py` (§7); disassembly by `scripts/tools/eedis.py`. Check output:
`lead_tmp/re/vdth/checks.txt` (§9).
**Conventions.** B1 = `analysis/SWBFRONT.ELF-decomp.c`, B2 = `analysis/SWBFRNT2.ELF-decomp.c` (line numbers); ELF VA
→ file offset = VA − 0x160000 + 0x1000. [V] read in the decomp; [V-instr] read in the disassembly; [V-RAM] measured
in PS2 RAM; [D] derived; [H] hypothesis. [O] = load-bearing, verifier-checked (§9). Class offsets are BF1 unless noted.

## 0. The answers

| # | question | answer |
|---|---|---|
| 1 | who is hit | every damageable object whose collision is in the gathered layers (B1 mask 0x81e: soldiers, vehicles, buildings, riders, …; B2 0x103b) within an axis-aligned box of half-side max(DamageRadiusOuter, PushRadiusOuter) around the blast, **at most 20**, each only if a ray from the blast to its target point is not blocked by **buildings or terrain** (B1 mask 0x48, B2 0x90) — soldiers and vehicles never shield each other (§2). |
| 2 | falloff | `d' = |sphere centre − blast| − w/2` (w = the victim's collision-sphere radius); `f = 1` if `d' < Inner`, `0` if `d' ≥ Outer`, else `(Outer − d')/(Outer − Inner)`; damage = `Damage × f`. `DamageRadius` sets **Outer**; Inner defaults to **0** (§1, §3). |
| 3 | w | a vehicle: `max(SPHR.r, class+0x1498)` about the `modl SPHR` centre, the second term = the farthest `AddSpringBody` / ODF collision primitive (reproduces all 16 BF1 vehicle classes in RAM exactly); a soldier: 1.4626 standing (centre feet + 1.0), 1.1023 crouched (+0.75), 1.6221 prone (≈ +0.35) [V-RAM] (§3). |
| 4 | team / scales | the same TakeDamage as a bolt: `× scale[HealthType]` (the explosion's Person/Animal/Droid/Vehicle/Building scales; `ArmorScale` sets Vehicle + the building scales), same team `× friendly-fire scale` (default 1.0), an AI's blast cannot hurt a human player's teammate at all, shields first (§4). A vehicle's own blast carries **its driver as attacker and the vehicle's team** (§4). |
| 5 | riders | a soldier riding a vehicle takes **no** splash (only one mounted on a building turret does) (§5). |
| 6 | push | only soldiers and droids react (every other class's push is a no-op); impulse `(normalize(centre − blast) + (0, 0.6, 0)) × Push × f(PushInner, PushOuter)`; BF1 pushes the living, **BF2 by default only the dead** (`PushDeadOnly`, default on) (§5). |
| 7 | chunks | launched from the vehicle's velocity × ChunkVelocityFactor plus an outward kick (§6.1); 40-kg rigid bodies under gravity with restitution = ChunkBounciness, spin friction = ChunkStickiness; after `ChunkTerrainCollisions` hits they settle, then vanish **3 s (BF1) / 8 s (BF2) after resting** (explosion debris: 5 s in both) (§6.2). |
| 8 | the riders | after the blast, every occupant of every seat is **killed** (not ejected) with **the vehicle's last-damage record**: the destroyer gets one kill per rider (a team kill if friendly), a crash with no attacker kills them with no killer; a ticket each; past the invulnerability switch. Exposed riders leave a corpse, riders inside the hull are deleted. Both games; live on BF1: 11 riders of 6 hovers (§10, card RIDK). |

## 1. The explosion class (both games) [V][O]

Registration: B1 `FUN_00172520` (`"explosion"`, 0x1c0 B) → ctor **FUN_0026ed88**; setter **FUN_0026f528**; explode
**FUN_002701a0**. B2 `FUN_00173710` (0x1f0 B) → ctor **FUN_0030b2f8**; setter **FUN_0030bb88**; explode
**FUN_0030c8f8**. Property names confirmed by FNV.

| property (hash) | B1 offset | B2 offset | default (ctor) |
|---|---|---|---|
| Damage 0x59e94c40, MaxDamage 0x47d0e794 | +0x30 | +0x3c | 0 |
| PersonScale / AnimalScale / DroidScale / VehicleScale | +0x34/+0x38/+0x3c/+0x40 | +0x40/+0x44/+0x48/+0x4c | 1.0 |
| BuildingScale 0x02168f61 (three building scales) | +0x44,+0x48,+0x4c | +0x50,+0x54,+0x58 | 1.0 |
| ArmorScale 0xa0bb60ce (= Vehicle + the three building scales) | +0x40..+0x4c | +0x4c..+0x58 | — |
| 0xc37970cf, unnamed (= Person + Animal + Droid) | +0x34..+0x3c | +0x40..+0x48 | — |
| ShieldScale 0xd0545c72 | +0x50 | +0x60 | 1.0 |
| **DamageRadiusInner** 0xded40df6 | **+0x54** | **+0x64** | **0** |
| **DamageRadius** 0xb68798a6 and **DamageRadiusOuter** 0xa23b939d | **+0x58** | **+0x68** | **0** |
| Shake / ShakeRadiusInner / ShakeRadius = ShakeRadiusOuter / ShakeLength | +0x68/+0x6c/+0x70/+0x74 | +0x78/+0x7c/+0x80/+0x84 | ShakeLength 0.5 |
| **Push** 0x876fffdd / **PushRadiusInner** 0x96301ec5 / **PushRadius** 0x19b5e2bb = PushRadiusOuter 0x71262ebe | +0x84/+0x88/+0x8c | +0x94/+0x98/+0x9c | 0 |
| HurtOwner 0x0be13b2f (flags bit 1) / **PushDeadOnly** 0x26d8bec1 (flags bit 0) | — | +0xa0 bits | **1 / 1** |
| Effect / Decal / WaterEffect / SoundProperty | +0x90/+0x94/+0x98/+0x9c | up to 5 Effects (+0xa4.., random pick) … | — |
| NumChunks 0xf54f2869 (byte), ChunkGeometryName, ChunkSpeed "min max", ChunkOmega, ChunkPhysics, ChunkTerrainCollisions, ChunkTerrainEffect, ChunkTrailEffect / ChunkSmokeEffect (×3), **ChunkStartDistance** 0xcd61e401 | +0xa4, chunk template at +0xb0 (§6.1), ChunkSpeed +0x1a0/+0x1a4 (−1/−1), ChunkStartDistance +0x1a8 | template at +0x130.., ChunkSpeed +0x1d0/+0x1d4, ChunkStartDistance +0x1d8, **ChunkGravity** 0xd0259d88 +0x160 (0, −9.8, 0) | — |
| LightColor / LightRadius / LightDuration | +0x1ac/+0x1b0/+0x1b4 | +0x1dc/+0x1e0/+0x1e4 | — |
| **VisibleRadius** 0x53a81b7f | (constant 60) | +0x1e8 | **60.0** |

Cites: B1 setter FUN_0026f528 — Damage/MaxDamage +0x30 (B1:193586; tests B1:193570, 193582), DamageRadius/Outer +0x58
(B1:193272; tests B1:193262-193268), DamageRadiusInner +0x54 (B1:193482), Push +0x84 (B1:193592-193593),
PushRadiusInner +0x88 (B1:193285-193286), PushRadius/Outer +0x8c (the final statement B1:193637, reached from
0x19b5e2bb at B1:193556 and 0x71262ebe at B1:193589), ArmorScale B1:193301-193304 (→ 0x0026f9a0), 0xc37970cf
B1:193259-193264, BuildingScale B1:193539-193541 → 0x0026f9a0. Ctor FUN_0026ed88: *(decompiler detail omitted)*
(+0x54/+0x58, B1:192900-192901), *(decompiler detail omitted)* (+0x84/+0x88/+0x8c, B1:192908-192910), *(decompiler detail omitted)*
(ShakeLength, B1:192894), the 1.0 loop over +0x34..+0x4c and *(decompiler detail omitted)* (+0x50) (B1:192875-192882). B2
setter FUN_0030bb88: Damage +0x3c (B2:300065), DamageRadiusInner +0x64 (B2:299784), DamageRadius/Outer +0x68
(B2:299922), PushRadiusInner +0x98 (B2:299869), PushRadius/Outer +0x9c (B2:300043), Push +0x94 (B2:299626),
VisibleRadius +0x1e8 (B2:300056), ArmorScale B2:299909-299917, HurtOwner bit 1 (*(decompiler detail omitted)*, B2:299933) /
PushDeadOnly bit 0 (*(decompiler detail omitted)*, B2:299984), both written at B2:299987; the derived-class ctor FUN_0030b628 copies
both bits from the parent class (B2:299387-299388, 299432-299433), so a child `_exp` inherits them. B2 ctor
FUN_0030b2f8: flags `|= 1` (B2:299247) and `|= 2` (B2:299307), VisibleRadius 60.0 (B2:299305), chunk gravity y −9.8
(B2:299295). The AI's damage estimate (B1 FUN_00237ff8 / B2 FUN_002a9e58: the class Damage × falloff at 2 m × shield
or `scale[HealthType]`, B1:151552 / B2:230085) independently confirms both scale tables (B1 +0x34 + 4·type, B2 +0x40
+ 4·type).

## 2. The blast — one function for every explosion source [V][O]

B1 FUN_002701a0(class, pos, attackerDesc16, impactNormal?) / B2 FUN_0030c8f8 (same order; callers: every ordnance
impact, mine, building and vehicle death — `re_check.py callers`, §9):
1. `visible` = a sphere of radius 60 (B1 constant, B1:193756 `FUN_0019fc10(0x42700000,…)`; B2 the class's
   VisibleRadius, B2:300188 `FUN_001ba840(class+0x1e8, 400.0, …)`) touches some camera's frustum. Only gates
   presentation: Effect (+ its sound), Decal (size = DamageRadius), Light, the water splash (when `pos.y ≤ water + 0.3`,
   B1:193763), and the shake (step 5). The SoundProperty sound plays always.
2. Chunks (`NumChunks` of `ChunkGeometryName`) — §6.1. Always.
3. **Gather** (only if DamageRadiusOuter > 0 or PushRadiusOuter > 0): `FUN_00161c00(max(DamageOuter, PushOuter), pos,
   list, 20, 0, 0x81e)` (B1:194072; B2 `FUN_00161a98(…, 20, 0, 0x103b)` B2:300525) → FUN_00162a98: an **axis-aligned
   box** pos ± r over the collision manager's layer lists of the mask (B1 lists 2, 4, 8, 0x10, 0x800 — re-collision-
   layers §4; 0x10 holds riders, §5), each body mapped to its owner, **cap 20**. Desc = [the 16-byte attacker record
   passed in][class Damage, the 7 scales, ShieldScale] (B1 copies class+0x30..+0x50 after it; the ordnance desc
   layout of re-blaster-damage §6.1).
4. For each gathered object (from the end of the list): target point `t = obj.vt+0x5c(obj, pos, centre − pos, −1)`
   (B1 FUN_001e2500 for most classes: the collision-sphere centre, or a point on the class's TargetableCollision
   bodies when it has any — FUN_00168030, not read [H]; the soldier's own FUN_0020c680 not read [H]);
   **LOS** `FUN_00161de8(pos, t, &obj, 1, 0x48)` (B1:194099; B2 `FUN_00161ce0(…, 1, 0x90)` B2:300680) — the object
   itself ignored, **only buildings and terrain block** (B1 0x48 = 8 | 0x40, B2 0x90 = 0x10 | 0x80); blocked → no
   damage and no push. Clear →
   - if DamageRadiusOuter > 0: `obj.vt+0x80($f12 = Inner, $f13 = Outer, obj, pos, desc)` [V-instr 0x271208-0x271238]
     (B2 `vt+0x8c`, same registers from +0x64/+0x68 [V-instr 0x30dd20-0x30dd50]);
   - if PushRadiusOuter > 0: `obj.vt+0x84($f12 = PushInner, $f13 = PushOuter, $f14 = Push, obj, pos)` [V-instr
     0x27123c-0x271264] (B2 `vt+0x90` [V-instr 0x30dd7c-0x30dda8]).
   - **B2 only:** a gathered object on collision layer 1 must pass `vt+8` IsA(DAT_006896d8) (0x30dc18-0x30dc3c);
     `HurtOwner = 0` skips the attacker's own object (the handle-checked `attacker+4`, B2:300612-300633);
     with **PushDeadOnly = 1 (default)** the push is skipped for an object whose health-flags alive bit
     (`obj+0x228` bit 3) is set (B2:300686-300690, [V-instr] `lbu 0xa0 / andi 1 … lw 0x228 / srl 3 / bne`).
5. Shake (only if `visible` and ShakeRadiusOuter > 0): for each of the two local players whose view has a controlled
   entity, `f` = the same falloff (§3) on the distance to that entity with ShakeRadiusInner/Outer; amplitude Shake·f
   (× a per-entity factor), length ShakeLength·f (camera FUN_0019fbb0) and pad rumble (FUN_003499b8, 1.2 × length)
   (B1:194113-194150; B2 the same, B2 FUN_00268fe0 at B2:300586).

## 3. The falloff and the victim's sphere [V][O]

**Splash** (B1 FUN_00221600, the vt+0x80 of 22 of the 23 damageable BF1 vtables, §9; B2 FUN_002690d0, 27 of 28):
`centre = the object's collision sphere` (entity+0x110 or its copy in the cell table `*(+0x100) + 0x40 + 16·(+0x104)`),
`f = falloff(|centre − pos|, Inner, Outer, obj)`, and if `f > 0` the desc's damage word is multiplied by f
(B1:135656) and TakeDamage runs (B1 `… B2
`FUN_001e0900(obj+0x170, …)`).
**Falloff** (B1 FUN_00221530, B2 FUN_00268fe0):
```
d' = d − w · 0.5                       (w = the object's sphere radius; B1:135586)
     B2: d' = d − w · class+0x37c · 0.5  (B2:185862; class+0x37c = 1.0 (B2:186501) except the one BF2 class
                                          authoring 0x9c1a4523: myg1_bldg_energy_collector_core 1.5)
f  = 1 if d' < Inner;  0 if d' ≥ Outer;  (Outer − d') / (Outer − Inner) otherwise          (B1:135588-135596)
```
With the disc's usual single `DamageRadius` (Inner 0): full damage while the blast is within w/2 of the sphere
centre, falling linearly to 0 at `DamageRadius + w/2`.
**w is a radius** [V]: FUN_00168830 builds the object's broadphase box as sphere.centre ± w. **Vehicles** [V][V-RAM]:
the EntityGeometry ctor FUN_001e1e18 sets `centre = M · SPHR.centre` and `w = max(model sphere r, class+0x1498)`
(B1:91424-91462). The model sphere is the `modl`'s **SPHR** chunk (tag 0x52485053: x y z r, B1 FUN_00497150
B1:558104-558118), overriding the INFO vis-box sphere (B1:558092-558096). `class+0x1498` grows by `|p − c| + e` for
each ODF collision primitive (`… aacylinder …` lines; e = r for a sphere (the default branch), `√(2r² + h²/4)` for the
cylinder kinds (kind hashes 0x4a182b8f and 0x73f51491, FUN_00444bb8's CRC), `√(hx²+hy²+hz²)` for a box (0x1ccafdcc);
FUN_001e3678 B1:93183-93266) and each hover `AddSpringBody x y z r` (0x5f1b7721, at most 6; FUN_001f0648
B1:102072-102125); the `prim`-chunk bodies do not grow it. **RAM check** (`veh_w.py` over 50 BF1 states; SPHR read
off SIDE/*.LVL): SPHR alone gives 12 classes exactly (all six flyers, AT-AT 15.9937, AT-TE 10.9674, AAT 7.5965 with
its −1.4745 m z offset, spider 15.1219, hailfire 7.3617, tauntaun 2.4406, droideka 1.8931); `AddSpringBody` gives the
speeder bike 3.1467, STAP 4.0043, fighter tank 6.3306; the AT-ST's `BldgCylinder aacylinder … 0 4.7 0 3.0 7.6` gives
6.39985 (RAM 6.39988).
**Soldiers** [V-RAM] (`soldier_w.py`, 2,652 soldiers in 55 BF1 states; the stance index at soldier+0xbe0): the same
for every class — standing w 1.462635 (0x3fbb379c) centre feet + 1.0; crouched 1.102270 centre + 0.75; prone 1.622066
centre ≈ + 0.35. The code that writes them was not found [H] (no ELF word holds the three values).

## 4. Damage application, teams, and the vehicle's own blast [V][O]

TakeDamage is the bolt's (re-blaster-damage §6.2): B1 FUN_001bc190 — `dmg = desc.damage × mult` (mult 1.0 from
FUN_001bc8e0, B1:64354 region); same team (`desc[1] == victim team`) → **× DAT_00517508** (ELF 1.0; its only writer
FUN_001bbaf8 is fed a game-option percent byte × 0.01, or in FUN_001bba58's fallback 1.0 / 0.0 by DAT_00518bd8 —
friendly fire on/off, §9) — but if the attacker is not a player (`attacker+0x9c < 0`) and the victim is a human player
(`+0xe8 ≥ 0`) the damage is dropped entirely; shield phase with ShieldScale; health phase `× desc.scale[HealthType &
7]`. (A global DAT_00519060, default 0, set only by FUN_00170840, drops all damage to human-controlled victims — an
invulnerability switch, not a rule.) B2 FUN_001dfb38 the same with DAT_005fcbe0 (ELF 1.0) and a victim-side
multiplier from `comp.vt+0x24` (FUN_001e0900). No per-part or distance term beyond §3.
**The vehicle's own blast** — B1 hover tick FUN_001ea9f8 (B1:97302-97306): `FUN_002701a0(class+0x2698, entity+0x230
(position row), {entity+0x380, entity team (+0xd8), class, 0}, 0)` then the chunks `FUN_001e2630(1.0, entity)`
(B1:97314). B2 FUN_0021b470: desc `FUN_001df050(1.0, &d, entity+0x328, entity, team (4-bit field at +0x268), class, 0)`
→ `FUN_0030c8f8(class+0xc88, entity+0x150, &d, 0, 0)` (B2:132028-132031), chunks B2:132040. So the blast is the
driver's (kill credit) on the vehicle's team: its own side takes friendly-fire-scaled damage [D]. Which object
+0x380 / +0x328 is when the vehicle is empty is not traced [H].
**Flyers** read their death explosion through a flyer-only key 0x8a323f32 (B1 FUN_001df3f0 B1:90023, B2 FUN_0020fd58
B2:125483; unnamed) — for card VFLY.

## 5. Soldiers and droids [V][O]

- **Riders:** the soldier's splash (B1 EntitySoldier vtable 0x532d50 +0x80 = FUN_00200e68; B2 0x61eb88 +0x8c =
  FUN_00239480) returns **without damage** when the soldier is mounted and the mount's collision layer is not the
  building layer (B1 `!= 8` B1:112821, B2 `!= 0x10` B2:152663); otherwise the common splash, and if the soldier is
  dead after it, a fling direction `normalize(soldier pos − blast)` goes to FUN_00200f90 / FUN_002395a8 (death
  animation / ragdoll [H]).
- **Push** — the base slot is `jr ra; nop` (B1 FUN_00222a60 +0x84, B2 FUN_0026baf0 +0x90; ELF words §9): vehicles,
  buildings, props and items are never pushed. Soldier (B1 FUN_002009a0 / B2 FUN_00238b80) and droid (B1
  FUN_001c8590 / B2 FUN_001ed7c8) only. Soldier: skipped in five soldier states (B1 soldier+0xbe4 ∈ {5, 8, 9, 12,
  13}; B2 +0x7e4 ∈ {9, 10, 11, 0x11, 0x12}) [H: which states]; a soldier whose +0x290 → +0x320 is set resists with
  probability class+0x1e58 (B2 +0x121c) — that class field defaults to −1.0, so the roll never succeeds unless hash
  0x43b50014 sets it; `f = falloff(|centre − blast|, PushInner, PushOuter)` when the distance > 1e-4; impulse
  direction `normalize(centre − blast) + (0, 0.6, 0)` (not renormalised) × `Push · f` [V-instr 0x200ba4-0x200c04] →
  B1 FUN_002008b0: velocity accumulator +0xa20 += impulse; if its y > 2.0 the soldier goes airborne (FUN_00209738),
  else if the horizontal speed exceeds DAT_00517874 (ELF 2.0) FUN_00209758(soldier, 8) (a stagger) [H: state names].
- **BF1 vs BF2:** BF1 pushes every gathered soldier/droid; BF2 with the default PushDeadOnly = 1 pushes only those
  the blast (or anything earlier) killed — a living BF2 soldier is not knocked back. No disc class authors
  PushDeadOnly 0 (one authors 1, §7).

## 6. Chunks [V][O]

### 6.1 The launch
**Vehicle CHUNKn** (B1 FUN_001e2630 / B2 FUN_00213630, same law): every slot with geometry (≤ 9 per class; defaults
from the class ctor FUN_001e30c8: ChunkSpeed −1, ChunkVelocityFactor 1.0, ChunkUpFactor 0.25, B1:92322-92337):
```
start pose = vehicle matrix (× the ChunkNodeName node's pose when set)
c          = centroid of all chunks' prim positions
d          = chunk prim position − c
dir        = normalize(d + (0, ChunkUpFactor·|d|, 0))                              B1:92028
v          = vehicle velocity × ChunkVelocityFactor                                 B1:91931
if ChunkSpeed ≥ 0:  v += dir × ChunkSpeed                                           B1:92043
else:
    if |d|² > 1:  v += dir × U(15, 30) / w_vehicle      (w = the vehicle's collision-sphere radius, §3)   B1:92059-92071
    if ChunkPhysics == 3:  v *= 0.25                                                B1:92088-92090
    if |v| < 5:  v = dir × U(4, 6);  if v.y < 2: v.y = U(2, 3)                      B1:92109-92126
v *= 1.0 (the death call's scale)
```
The cluster is tagged with the vehicle class (`cluster+0x54a0 = class`, B1:91882).
**Explosion NumChunks** (in FUN_002701a0, B1:193847-194058; ≤ 10): each at the blast with a uniformly random
orientation; velocity with no impact normal (the vehicle-death case): ChunkSpeed set → `U(min, max) × normalize(2(½−U),
U+½, 2(½−U))` (upward-biased); unset → `(PushRadius + U·PushRadius) × normalize(2(½−U), 4(U+1), 2(½−U))`; the chunk
starts `ChunkStartDistance` along its velocity direction (B1:194039); cluster tag 0 (B1:193847). The explosion class's
chunk template has ChunkBounciness 0, ChunkStickiness 0, both dampings 0 and ChunkLifetime −1 (ctor B1:192934-192951;
ChunkGeometryName handler B1:193340-193353) — explosion debris does not bounce.

### 6.2 The body (B1 chunk init FUN_001b80d0, tick FUN_001b8c60, contacts FUN_001ba570; B2 FUN_001d6608 …)
- A rigid body of **mass 40** (inverse 0.025, B1:61235-61236), inertia from its prim × 40, restitution = ChunkBounciness
  (default 0.6), friction = ChunkStickiness (default 0.7), linear / angular damping = ChunkLinearDamping 0xfeb5ed07
  (0.001) / ChunkAngularDamping 0x0e3bec8e (0.004) (vehicle slot defaults B1:93006-93010).
- Tick (B1:61871-62100): force = mass·g − LinearDamping·v (B1 g = (0, −9.8, 0) literal, B1:61875; **B2 the chunk's
  ChunkGravity vector**, B2:84015-84024); `v += F·(1/40)·dt`; spin: angular momentum decays by AngularDamping, the
  orientation rotates by ω·dt; `pos += v·dt`.
- Contacts (FULL, FUN_001ba0b0): soldiers ignored (layer 2, B1:62781; the FULL chunk's soldier subset is also cleared,
  B1:61548-61550); a normal impulse `j = −(1 + e)·vₙ / (1/m + …)` (B1:62892) and a friction impulse on the spin only
  (B1:62979); each contact counts. When the count reaches **ChunkTerrainCollisions** on a terrain contact the
  ChunkTerrainEffect plays there (FUN_001b9a70, B1:62858) and later terrain contacts get no impulse (vehicle and
  building contacts still bounce). SIMPLE (FUN_001b9ad8) bounces the linear velocity only, and kills a soldier or
  vehicle it hits faster than ChunkDeathSpeed 0xed35b79b (default −1 = never; B1:62546-62558).
- **Settle** (B1:62300-62349): once the count exceeds the budget, a STATIC (mode 2) chunk settles at once; any other
  settles when a downward ray (length r·1.001, building | terrain) hits within 0.75 (0.3 for mode 4), or when the count
  exceeds budget + 5 (moving flag cleared, FUN_001b8080).
- **Lifetime** (B1:61846-61866): the timer runs only while resting (or on a map without terrain, TerrainEnable 0, or
  a 0xff budget); life = ChunkLifetime if ≥ 0, else **BF1 3 s** (FULL/SIMPLE) / 53 s (STATIC, mode 4) / 30 s (budget
  0xff) — **BF2 8 s** / 53 s / 30 s (B2:83976-83987). At rest the timer starts at 0 for a vehicle cluster; for an
  explosion cluster (tag 0) B1 sets −2.0 [V-instr 0x1b983c-0x1b9848] and B2 +3.0 (B2:84519) — so explosion debris
  rests **5 s** in both games, vehicle chunks 3 s (BF1) / 8 s (BF2). A chunk whose sphere falls below the terrain with
  no building beneath (256 m ray) is removed (B1:62362-62373).
- Pool: 8 clusters × 10 chunks (FUN_001bb0b8); a new blast with no free cluster reuses the next one round-robin,
  killing its chunks.

## 7. The disc (vehicle `_exp` classes, `census.py`) [V]

| class (both games unless noted) | _exp Damage / DamageRadius (Inner–Outer) | scales | Push / PushRadius | the _exp's NumChunks (the vehicle's own CHUNKn are separate) |
|---|---|---|---|---|
| imp/rep speeder bike, STAP, skiff (BF1) | 20 / 10 | Armor 1, 0xc379 1 | 2 / 3 (imp) or 5 | 8 / 6 / 10 |
| AT-ST (all three) | 50 / 20 | 1 / 1 | 13 / 7 | 0 |
| AAT, imp fighter tank, combat speeder / landspeeder | 100 / 4 | **VehicleScale 0** | 5 / 5 | 12 |
| rep fighter tank | 400 / 4.5–6.5 | VehicleScale 0, BuildingScale 0 | 10 / 4.5–4.5 | 12 |
| hailfire, snail tank (BF2), spider walker | 40 / 30 | Armor 3, 0xc379 0.8 | 13 / 29 | 7 / 7 / 0 |
| AT-TE | BF1 40 / 30 (Armor 3); BF2 40 / 15 | 0xc379 0.8 | 13 / 10 | 0 |
| AT-AT | 400 / 50–55 | 1 / 1 | 13 / 50 | 0 |
| MTT, hovernaut (BF1) | 50 / 20 | 1 / 1 | 13 / 7 | 12 / 15 |

The unnamed 0xc37970cf (Person + Animal + Droid) and ArmorScale (Vehicle + buildings) are the usual pair. HurtOwner 0 on
two BF2 expc classes (Sidious's and Mace's hero powers), PushDeadOnly 1 on one (Force Push); no vehicle `_exp` authors
either.

## 8. For the port (card VDTH build)

- **core** (a sim change: damage is state): `Event::VehicleDestroyed` already carries class, pos, vel, killer; add a
  blast step that looks up the class's `_exp`, gathers units and vehicles within the box (cap 20), LOS against
  buildings and terrain only, applies §3's falloff with each victim's sphere and §4's TakeDamage (`Health::
  take_damage` with the explosion desc); riders of a vehicle skip; one function usable by any ordnance explosion.
  Soldier sphere per stance (§3, measured); vehicle sphere per class (§3 rule; the bake must read `modl SPHR`, the ODF
  collision primitives and AddSpringBody — the RAM table is the test oracle).
- **Push:** BF1 knockback of living soldiers is feel work beyond this card — new card KNOCK (value 3). BF2's default
  pushes only corpses (no ragdoll in the port yet) → nothing to do until then.
- **game** (presentation): chunks as cosmetic rigid bodies (§6; the PS2's contact response against vehicles was not
  traced — chunks do not affect the sim, a reversible call); effects as the existing placeholder flash (`game/src/
  fx.rs`) until card FXC; the 60 m visibility gate is a PS2 limit the port drops (effects always spawn).
- **declares:** `sim` (a blast damages units near a destroyed vehicle; the arena/sweep never destroy a vehicle → expect
  no referee move); goldens none (no golden destroys a vehicle).

## 9. Checks run, the verifier, what stayed [H]

Checks (`lead_tmp/re/vdth/checks.txt`): `re_check.py fn` on every cited line (all in the named functions); `absent` on
the splash / falloff / override functions (only the listed users; the other falloff users are the AI estimates);
ELF vtable scans (BF1: 23 vtables hold FUN_00221600 at +0x80 and one, EntitySoldier 0x532d50, overrides it; +0x84 is
FUN_00222a60 in 22, FUN_002009a0 (soldier), FUN_001c8590 (droid); BF2: 28 hold FUN_002690d0 at +0x8c, EntitySoldier
0x61eb88 overrides; +0x90 FUN_0026baf0 in 27, FUN_001ed7c8 in one); `word` on DAT_00517508 (1.0), DAT_005fcbe0 (1.0),
DAT_00517874 (2.0), the slots and the `jr ra` bodies; `writers` on the FF scales (one each: FUN_001bbaf8 /
FUN_001df3c8, fed by the game options), the B2 flag word (ctor, setter, and the derived-class copy the verifier
found), VisibleRadius; `callers` of both explode functions, both chunk launches and ticks.
**Verifier** (one Opus agent, foreground, claims and citations only; 138 tool uses;
`lead_tmp/re/vdth/verify/VERDICTS.md`): **23 SUPPORTED, 4 PARTLY, 0 NOT SUPPORTED** over C1-C27. The PARTLY four and
its notes are folded in above: the BF2 derived-class flag copy (C3), the friendly-fire scale's second source and
DAT_00519060 (C16), the soldier push gates (C12), the mode-2 settle (C22); plus BF2's layer-1 IsA filter, the extra
extent branches and the 6-spring cap.
**[H]:** the soldier's stance spheres' writer; FUN_00168030's target point; the soldier's LOS point FUN_0020c680;
the soldier push's excluded states and resist classes; the airborne / stagger state names; what +0x380 / +0x328 hold
on an empty vehicle; the chunk-vs-vehicle contact response; flyer death key 0x8a323f32; that PushDeadOnly's default
really leaves living BF2 soldiers unpushed in play (a PCSX2 check belongs to card KNOCK).

## 10. The riders and the kill credit (card RIDK) — OPUS RE (2026-10-16) - [O] UNAUDITED

**The question** (card RIDK; `re-vehicles.md` §6's "killed or ejected [H]"): when a manned vehicle's hull pool runs
out, what happens to the soldiers in its seats, and who gets the kill? The port killed every rider as a WORLD death
with no killer (`Sim::destroy_vehicle`, PROVISIONAL since VDTH); on main 78-80 % of all world deaths were those riders.
**Answer.** In both games every occupant of every seat is **killed**, not ejected, and the death goes through the
ordinary member death with **the vehicle's last-damage record as the killer**: whoever dealt the hit that destroyed
the vehicle is credited with one kill per rider (a team kill for a friendly destroyer, a suicide for a rider who
destroyed his own ride). A vehicle whose last damage had no attacker (a crash into terrain or a building) kills its
riders with no killer. Each rider's death costs his team a ticket as any death does. Riders sitting exposed leave a
corpse; riders inside the hull vanish with the vehicle.

### 10.1 The path, BF1 [V][O]
1. **The hover tick** FUN_001ea9f8, the first update the vehicle's alive bit (E+0xac bit 3) is clear and its latch
   E+0x2148 is 0 (B1:97273): if the record's attacker E+0x24 is set, `FUN_002a5c28(stats(attacker))` — the
   destroyer's vehicle-kill count +1 (B1:97284-97289); the `ExplosionName` blast with {E+0x380, team E+0xd8, class, 0}
   (B1:97295-97306, §4); the chunks (B1:97314); then **the health interface's slot +0xc on E+8** (B1:97315).
2. That slot is the vehicle's death function: the hover's health vtable 0x5315a8 +0xc = thunk FUN_001f2690
   (B1:102974-102979) → **FUN_001ea190** (`re_check word` 0x5315b4 = 0x1f2690; the Controllable vtable 0x531468 +0xc
   = FUN_001f22a8 → the same function, B1:102739-102744). FUN_001ea190 (B1:96767-96808) sets the latch, clears the
   alive bit, calls slot +0xc of every **MountedTurret** seat (E+0x650.., count byte E+0x670, B1:96783-96791) and every
   **PassengerSlot** seat (E+0x640.., count byte class+0x2674, B1:96795-96803), then **FUN_0023ac68 on the driver's own
   seat** (E+0x2a0, B1:96806). The two seat kinds' slot +0xc (vtables 0x534310 / 0x534728, words 0x53431c = 0x2268a0
   and 0x534734 = 0x2283c8) end in … it is the base Controllable's slot +0xc
   (vtable 0x535f50, word 0x535f5c). The walker's (FUN_00217040, B1:128246-128336) and the flyer's (FUN_001d8398,
   B1:84349-84426) death functions run the same loops: **every vehicle kind**.
3. **FUN_0023ac68(seat)**, a seat whose pilot kind seat+0xf0 is 2 (a soldier in a vehicle seat), offline
   (disassembly 0x23ac68-0x23aeec; the network-client gate DAT_00518be4 / DAT_00518be8 / DAT_00518c08 is 0 in the ELF,
   DAT_005e9a70 is .bss, all written only by the network code):
   - s3 = (the rider's collision layer == 0x10, the riders' layer) — read BEFORE the kill [0x23ad74-0x23ae00];
   - **`FUN_001bbc98(rider + 8, vehicle + 0x24, 0)`** [0x23ae04-0x23ae34]: the kill, with the VEHICLE's record — run
     when the seat's slot +0x18 (its vehicle) is non-null, which it always is for a vehicle seat (a null skips straight
     to the next step, 0x23ad88);
   - the rider's slot +0xc = his death function FUN_001ffec0 (soldier Controllable vtable 0x532be0, word 0x532bec =
     thunk 0x210c30) [0x23ae38-0x23ae44];
   - unless s3: the rider's slot +0x8 with 3 = the soldier's deleting destructor FUN_001ff8b8 (word 0x532be8 = thunk
     0x210c10; frees to pool 0x586298 when bit 0, B1:111948-111950) [0x23ae90-0x23aea4]: **no corpse**.
4. **The kill** FUN_001bbc98 (B1:64129-64190): if the victim is alive it copies the 16-byte record into the
   victim's own record (health+0x1c = soldier+0x24, B1:64140-64143); the third argument 0 skips the invulnerability
   switch DAT_00519060 (the test sits inside *(decompiler detail omitted)*, B1:64158-64159) — **an invulnerable player in a
   destroyed vehicle dies too**, as in a death region (`re-kill-floor.md`); health 0 and the on-damaged slot.
5. **The record** is the last hit's: TakeDamage FUN_001bc190 copies its desc into health+0x1c only while the victim
   is alive and when the hit took shield or health (B1:64439-64442). The killing hit always does both, so **vehicle+0x24
   at death = the killing hit's desc** {attacker member, attacker team, weapon / ordnance words}. The two writers read
   here (FUN_001bc190, FUN_001bbc98) both test the alive bit, so the record stays in place after the death [H: other
   writers of health+0x1c not searched]; in all 11 live cases the riders took the record the vehicle held in its
   death update (§10.3).
6. **The rider's death** FUN_001ffec0 (B1:112066-112305): on layer 0x10, FUN_00200f90 (B1:112292-112293) exits the seat
   (Exit slot +0x54 with the vehicle's alive bit) and — only when the seat's slot +0x70 returns 0 — throws the body at
   (0, U·0.3 + 0.4, 0) × 9 (`PilotDeath = "fly"`: the direction DAT_0066eee0 is 0 in .bss, written only to 0); state 3
   (dead) (B1:112297-112300); then FUN_0023ac68 on the rider's OWN seat (B1:112302), whose pilot kind 1 (self) runs
   **FUN_00297440(member)** [0x23ad14-0x23ad70].
7. **The member death** FUN_00297440 (B1:222070-222170), its only caller (1 `jal` in the ELF, no data word
   [receipt:3e407ebe] [receipt:53885b7f]): attacker A = the dead soldier's record word +0x24 (B1:222097-222099);
   A = 0 → a death with no killer (kind 3); else the team relation byte decides FUN_002a5930 (an enemy kill: kills +1,
   streak, per-class tally) or FUN_002a5b78 (an ally: kills +1 and team kills +1) (B1:222106-222113); the kill line on
   the HUD when a player is involved (FUN_00307940, kind 0 self / 1 team / 2 enemy, B1:222141-222151); the victim's
   deaths +1 (FUN_002a5da0, B1:222155); respawn timer 4 s; team alive −1; **tickets −1** (B1:222156-222168).
   Stats: `stats(m) = *(*(0x518388)+0x40) + ((m − *(mgr+0x3c))/0xe8)·0x1d0`, side = 0 for team 1, 1 for team 2, 0 for
   any other team (FUN_002a7590: `team − 1` as unsigned, > 1 → 0): kills byte +4, team kills +6, vehicle kills +8,
   deaths +0x2c (FUN_002a5930 / 5b78 / 5c28 / 5da0, B1:232674-232852).
8. **PilotDeath** (seat class +0x218; "fly" → 0, "fall" → 1, B1:156037-156054) is read by the seat's slot +0x70: a
   MountedTurret returns its class's value (FUN_00225978, turret class +0x50 + 0x218); the hover's own seat and the
   PassengerSlot return 1 (FUN_0023e8d0) — on a hover only a gunner can be flung.
9. **Crashes.** The hull impact damage FUN_001ee3e0 (B1:99903-100017) fills the desc with the OTHER body's controller
   (desc[0] = its seat's member, desc[1] its team) when the other body is an entity with a seat (slot +0x68 non-null:
   a soldier or a vehicle), else zeros: a crash into terrain or a building leaves an attacker-less record, a ram by a
   manned vehicle credits its driver. (The port deals hull damage only against the world; no vehicle-vehicle ram damage.)

### 10.2 BF2 — the same machinery [V][O] (twin claims checked by two readers, §10.4)
Hover tick FUN_0021b470 (B2:131921-132080): the vehicle-kill award FUN_00269a08 when the record E+0x190 is set
(B2:131997-132016), the blast with {E+0x328, team, class} (B2:132029-132031), the chunks, then the health slot +0xc on
E+0x170 → thunk 0x2247c8 → **FUN_0021a840** (B2:131271-131313): turret seats E+0x520 / count E+0x540, passenger seats
E+0x510 / count class+0xc64, each slot +0x1c (→ FUN_0026c6d0 / FUN_002719e0 → FUN_002f4e00(seat), B2:188232, 192302),
then **FUN_002f4e00(E+0x270)**, BF1's FUN_0023ac68 (base Controllable 0x625bb8 slot +0x1c): the kill through the
rider's health slot +0x20 = FUN_001df640 with **the vehicle's record E+0x190** and third argument 0 (B2:283473; the
cheat test DAT_005fcbd8 sits inside *(decompiler detail omitted)*, B2:91110-91111); the riders' layer is 0x20 (B2:283468); the
member death FUN_0034f650 (its only caller, [receipt:1c47752a]) reads the record at entity+0x190 (B2:345910-345912).
One difference: BF2's TakeDamage copies the desc into the record (gated on the alive bit latched at entry,
B2:91280, 91314-91315) BEFORE the damage and the friendly return (B2:91317-91325), so a zero-damage hit also takes the
record; earlier returns skip it (FUN_004241a0, the DAT_005fcbd8 cheat, the DAT_00600b1c list, a time check against
DAT_005fcbdc, B2:91261-91312). The killing hit still writes it last.

### 10.3 Live and RAM [V-RAM][V-live]
- **Census** (`engine/artifacts/lead_tmp/ridk/tools/riders_layer.py`, 153 BF1 save states): every soldier in the
  collision world whose member sits in a vehicle seat (173) is on list 0x10, type 0x10, state 12. Riders INSIDE a hull
  are on no list (a census by the world cannot see them; the live trial does).
- **Live** (`tools/ridk_probe2.py` on BF1 RHN1 and NAB1: every manned hover set to 1 HP, one row per game update; read
  by `tools/ridk_ana2.py`; outputs `out/ridk2_rhn1.json`, `out/ridk2_nab1.json`, a first run `out/ridk_rhn1.json`): **6
  hovers destroyed, 11 riders**. In the update after each death every rider's record equals the vehicle's (11 / 11),
  every rider's member deaths +1 (11 / 11), the riders' team −1 ticket per rider; in each of the **5 deaths with an
  attacker the destroyer's kills byte rose by the number of riders and its vehicle-kill byte by 1** within 0.1 s
  (all five enemy kills; in the first run a team-1 tank gunner destroyed a team-1 tank and both riders' records took
  his team-1 record — friendly fire is credited the same way); the death with an empty record (NAB1, an AAT, no
  attacker) credited nobody. The 4 exposed riders (state 12, layer 0x10: three Republic fighter tank gunners and one
  rider who had moved from that gunner seat to the driver's) became state-3 corpses on layer 2, removed 3.94-4.00 s
  later; the 7 inside the hull (both AAT seats, the fighter tank's driver: state 0, layer 2) left the soldier vtable in
  the death update or the next.
  Classes: `bd512283` rep_hover_fightertank, `bbd03a72` cis_hover_aat (fnv).

### 10.4 Checks, verifier, what stays [H]
Checks (`engine/artifacts/lead_tmp/ridk/checks.txt`): `re_check fn` on every cited line; `writers` on the five B1 and
five B2 gate globals; `word` on every vtable slot and gate default; `callers` of FUN_00297440, FUN_0023ac68,
FUN_001ea190, FUN_0034f650, FUN_0021a840, FUN_002f4e00; `absent` with `--elf` for the jal / data words of FUN_00297440
[receipt:3e407ebe] [receipt:53885b7f], FUN_0034f650 [receipt:1c47752a] and FUN_0021a840 [receipt:3e3f6d71].
Verifier: one workflow (`ridk-re-verify`, 7 agents at medium effort; claims and citations only,
`engine/artifacts/lead_tmp/ridk/verify/claims.md`, verdicts `verify/verdicts.json`): of 28 claims, the 18 read once —
15 SUPPORTED, 3 PARTLY, folded in above (the kill sits inside the vehicle-getter test; the stats side is 0 for teams
other than 1-2; the live corpse split is 4 / 7, not 3 / 8 — a tally bug in `ridk_ana2.py`, fixed and re-run); the 10
absence and twin claims each read by two separate verifiers — 9 SUPPORTED by both, 1 PARTLY by both (BF2's record copy
is skipped by four earlier returns, folded in); the second reader overturned none. (A first launch that sent the
verifiers an empty claim list was stopped; its 7 agents' answers were not used.)
**[H]:** which seats put their rider on layer 0x10 (exposed) — not traced; the live split is the Republic fighter
tank's gunner exposed, the AAT and the fighter tank's driver inside; a rider who moved from an exposed seat to the
driver's seat stayed exposed (both live runs). BF2's corpse split (layer 0x20) is read from the code only. The HUD kill
line's two record words (weapon / ordnance) are not named.

### 10.5 For the port (card RIDK build)
- `Sim::destroy_vehicle(h, killer)`: `killer` is already the killing hit's attacker (`damage_vehicle_by`'s shooter) =
  the PS2's record; a hull impact passes `None` = the PS2's zero record. No new vehicle state.
- Order, as the PS2: the `VehicleDestroyed` event; the blast with the pilot as attacker while the riders are still
  seated (riders take no splash, §5; the dying vehicle must not be gathered); then each seat's occupant (turrets,
  passengers, the driver last) dies: `Some(k)` → the bolt kill path (`unit_killed(u, Some(k))`: `Event::Kill`, k's
  kills, team / self kill counters); `None` → the attacker-less death (`EnvDeath`, `DeathCause::VehicleDestroyed`).
  Both cost a ticket. The `invincible` cheat does not spare a rider in either case (third argument 0, both games):
  `DeathCause::VehicleDestroyed` leaves `cheat_spares`.
- A destroyer who died before his riders' deaths is still credited on the PS2 (the member keeps the stats);
  `unit_killed` counts `kills` only for a living killer (`unit_mut`) — count it on the slot by generation instead, or
  record the deviation.
- Presentation (game layer, optional): exposed riders leave a corpse at their seat (fall; a turret seat with
  `PilotDeath = "fly"` throws it up at 3.6-6.3 m/s); riders inside the hull leave none.

## 11. What the rider's player sees when the hull dies

§10 says riders inside the hull vanish with the vehicle. The sim had that (`body` false), but the draw rule drew any dead
unit with a pose, so a rider killed inside a hull fell out of the air at his seat and played a death clip, on every vehicle
kind. `SoldierVisuals::sync_units` now marks a unit that died seated with no body as gone.

The camera (lines of `SWBFRNT2.ELF-decomp.c`):

- The unit object's destructor `FUN_001cdfc8` (76680-76695): a viewport whose player camera follows this object gets
  `FUN_001bb4c0(camera, 0)`, which clears the followed object (63784-63800). A death camera on this object is restarted with
  no unit.
- The player camera's apply with no object copies its stored matrix and sets the zoom divisor to 1 (63858-63862). So the view
  holds where it was.
- With no object the update only looks for one in the branch guarded by `DAT_00600b14` or the camera's `+0xc4` (63588, the
  random pick at 63729-63770). Both are 0 in save states 9 and 10, so that branch is off in this play.
- In save state 10, flying, the camera at `0x1759be0` follows `0x1cf2e58`, which is the flyer `0x1cf2bd0` + 0x288, so the
  object it clears on is the flyer's and not the pilot's.
- The death camera did start in a real crash (below), though no caller of `FUN_001ce0d8` is in the flyer. `trace:` which caller,
  and what puts the player camera back as current: the setup makes the death camera current (62971-62985).

The port holds the last seat camera while the rider is dead with no body (`WalkSim::held_seat_view`). The field of view stays,
since nothing sets one. It applies to every vehicle kind; only the flyer's camera object was read from RAM, so `measure:` a
hover and a walker. An exposed rider keeps a corpse and the ordinary dead view.

Measured in a crash, save state 10 of 14:15, 0.167 s after the death (member timer `+0x158` 3.833 of 4.0):

- Current camera `vp+0x24` is the player camera; its followed object is 0; `+0x90` reads 0.167.
- The player camera's rows are the chase view, the rotation within 0.03 of the state 0.65 s earlier, and the eye is at (91.76, 108.17,
  2217.42), not at the wreck.
- The death camera holds the same matrix, unit 0, timer 0. Its `+0x60` is the ship's last place + 0.8 up and its `+0x50` follows the
  set-up's rule, so it was set up on this crash and then restarted with no unit by the destructor. Timer 0 means it is not updating.
- The ship's object has its destructor's vtable (`+0x288+0x1c` = `0x618e08`), alive bit clear, health -1340.6 of 2250, velocity
  unchanged by the hit, as the rule that a dead craft takes no response says (`re-space-assault.md` §11).

`measure:` this is one instant. Nothing here shows the view staying for the whole 4 s.

### The explosion was never loaded

Flyers name their blast with `ExplosionDestruct`, hash `0x8a323f32`. The BF2 flyer setter stores it at class `+0xedc` (124798-124808) and the
death reads that slot and plays it (`FUN_0030c8f8`, 116897-116902); BF1's stores `+0x269c` (B1:89800, 89894-89903) and plays it
with `FUN_002701a0` (B1:85098-85103). The port read only `ExplosionName`, so every flyer on both games had no explosion: no effect
and no area damage. On the discs 12 BF1 and 35 BF2 `flyer` classes and 4 BF2 `commandflyer` classes carry the key and none carries
`ExplosionName`. `explosion_name` now takes the key by engine base. In the replay the burst fills the view with rays, as in the frame
of that save state, which is the only comparison made.

A ship's pieces are drawn lit and fade over their last two seconds: `re-chunks.md` §8.

`trace:` the three other holders of this key: `cis_hover_aat` and `cis_tread_hailfire` in both games, and `cis_hover_mtt` in BF1. They
stay on `ExplosionName`, which they lack, so they may have the same gap. The base setter's `+0x3ec` slot (B2:80167-80179) is unread.

Not ported: a ship shot to 0 health falls before it explodes on the PS2, the pilot inside. The alive bit clear in a state other
than 4 or 5 starts phase 4 with a timer (B2:116868-116935: 600 s when `FUN_00215f80(class)` is below 1, else 0.2 to 2.2 s);
expiry or any contact gives phase 5 (B2:118283-118286, 121913-121930, 119844), which is the explosion. `destroy_vehicle`
explodes at once. A crash explodes at once in both. `trace:` also the viewport `+0x90` factor in those phases (B2:122650-122700).

## 12. Ground/space death presentation and damage blink audit (FUN_001dfb38)

Audit of rider corpse visibility and red damage blink against `SWBFRNT2.ELF-decomp.c`:

1. **Rider corpse visibility during 4s held death camera:**
   - **Enclosed riders:** When a vehicle with enclosed seating is destroyed, `v.body` is marked `false`. On the unit death path, `SoldierVisuals::sync_units` detects `v.seated && !u.body` and marks `v.gone = true`, completely omitting the rider corpse from rendering during the 4s held death camera.
   - **Exposed riders:** For exposed mounts (e.g. speeder bikes, STAPs, open turret chairs), `v.body` is marked `true`. The soldier dies and leaves a ragdoll/corpse at the seat that falls under gravity.

2. **Vehicle red damage blink:**
   - In `SWBFRNT2.ELF-decomp.c` lines 90720–90755 (`FUN_001dfb38`):
     Damage taking > 2% remaining health flashes the vehicle red ONLY when the alive bit (`*(byte *)((int)pfVar15 + 0xb8) & 8`) is set.
   - When health reaches 0, the alive bit is cleared immediately.
   - Neither ground vehicles nor flyers blink red after destruction during the held death camera. Both ground craft and flyers follow identical alive-bit gating.
