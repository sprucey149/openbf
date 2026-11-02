# RE: Blaster weapon fire + damage (infantry rifles), BF1 & BF2

Status: mechanism recovered from both decomps, 2026-08-30 (P4 combat prep).
Sources: `analysis/SWBFRNT2.ELF-decomp.c` (BF2, primary), `analysis/SWBFRONT.ELF-decomp.c`
(BF1), raw ELFs in `extracted/` for vtable dumps and disassembly of small
functions Ghidra missed (capstone). ODF data from `wpnc`/`ordc`/`expc` chunks in
SIDE LVLs (parsed directly; note `bake classes` currently only reads `entc` —
weapon/ordnance/explosion classes live in separate chunk tags with the same
BASE/TYPE/PROP layout).

Markers: **[V]** = read directly from decomp/disasm or byte-verified data;
**[H]** = inference, needs PCSX2 confirmation. Hashes are fnv1a_lowercase
(`c|0x20`).

---

## 1. Big picture

Both games share one architecture (BF2 is a refactor of BF1 with data-driven
extensions):

- Weapon classlabel `cannon` (all blaster rifles/pistols) → engine class
  **WeaponCannon** (name embedded after the vtable: BF1 vtbl @0x0053b238,
  BF2 vtbl region @0x00628338..0x62844c, string "WeaponCannon" @0x628450). **[V]**
- Ordnance classlabel `bolt`/`laser` → **OrdnanceLaser**, `bullet` →
  **OrdnanceBullet**, `shell` → **OrdnanceShell** (registration: BF1
  FUN_00172568, label strings @0x526fb8..0x527038; "laser" and "bolt" both map
  to the OrdnanceLaser ctor FUN_0027c9d0). **[V]**
- Fire is a dt-driven accumulator state machine on the weapon instance
  (refire timer / salvo counter / salvo timer), run inside the frame update —
  NOT fixed-tick; multiple shots can be emitted in one update if dt spans
  several periods (loop consumes dt in chunks, epsilon 2^-15 = 3.0517578e-05). **[V]**
- A blaster bolt is a **simulated projectile with a single predictive raycast
  at spawn**: one ray covering the entire remaining flight path is cast when
  the bolt is created; the hit fraction converts to a time-to-impact, and the
  bolt simply flies until that time, then applies damage to the remembered
  object (id-revalidated). It is NOT hitscan (travel time is real) and NOT
  per-tick collision (objects entering the path after spawn are never hit —
  see §5 caveat). **[V]**
- Damage = ordnance `MaxDamage` × per-victim-HealthType scale
  (PersonScale/DroidScale/...), applied to a health component
  (`entity+0x170` in BF2). BF1 has **no distance falloff**; BF2 adds a
  **flight-time-based linear falloff** (full → end damage). No per-bodypart /
  locational multipliers exist in either game. **[V]**
- The soldier `AimFactor*`/`AimValue` ODF props **do not exist in either
  engine binary** — spread comes exclusively from weapon-class keys (§4.4).

---

## 2. Function maps

### BF1 (SWBFRONT.ELF) — all **[V]** unless noted

| addr | role |
|---|---|
| FUN_002864f0 | WeaponClass (base) ReadProperty — Ordnance/ammo/heat/zoom/range/targeting keys (§7.1) |
| FUN_00289b48 | WeaponCannonClass ReadProperty — ShotDelay/salvo/spread keys (§7.2) |
| FUN_002899c0 / FUN_00289a58 | WeaponCannonClass ctor (defaults: ShotsPerSalvo 1, SalvoCount 1, +0x228=2.0) / copy-ctor; class size 0x250 |
| 0x0053b238 | WeapCannon instance vtable (trailing string "WeaponCannon"); slots below |
| FUN_00288858 | cannon weapon-instance ctor: spread state init (human: −SpreadThreshold; AI: 0), multipliers 1.0 |
| FUN_00288930 | vtbl+0x0c per-tick: spread recovery `state −= dt·SpreadRecoverRate`, floor −SpreadThreshold (human) / 0 (AI) |
| FUN_002889e8 | vtbl+0x40 GetYawSpread = max(state,0) + postureBase + YawSpread; AI distance-dispersion table (§4.3) |
| FUN_00288c20 | vtbl+0x44 GetPitchSpread (same, PitchSpread) — Ghidra missed it; disassembled @0x2889b0-region |
| FUN_002889b0 | vtbl+0x48 SetPostureSpread(moveMag, posture, zoomed) → weapon+0x15c (§4.2); disassembled |
| FUN_00288c48 / FUN_00288c50 | vtbl+0x38/+0x3c: set spread multiplier weapon+0x154 / AI dispersion multiplier weapon+0x158 |
| FUN_00288c58 | vtbl+0x74 trigger-press init: charge frac +0x140 = pressed/MaxPressedTime; refire +0xd4 = ShotDelay; salvo reset |
| FUN_00288d10 | vtbl+0x78 **fire cycle** (dt): refire/salvo/ammo/heat/spread-growth loop (§3.1) |
| FUN_002892a8 | shot spawn: fire-point frame, salvo tilt, spread randomization (LCG), ordnance spawn packet, kick, net split (§3.3) |
| FUN_00289150 | vtbl+0xa8 fire-input predicate: charge→fire on release (cmd==−1); TriggerSingle→fresh press; else held (§3.2) |
| FUN_00289110 | charge-weapon variant of input predicate |
| FUN_002891c0 | "ready to fire" query (salvo done + refire elapsed + gates) |
| FUN_00204748 | SOLDIER per-tick: calls weapon vtbl+0x48(max(|fwd|,|strafe|), stance(0-2, >2→0), soldier+0x2c0 zoom flag) |
| FUN_00204578 | soldier fire processing (skipped in stance 3/5/0xa/0xb per re-stance-roll); maintains zoom state +0x2c0 |
| FUN_00172568 | ordnance classlabel→factory registration ("bullet"=0xdc cls, "shell"=0xe0, "laser"/"bolt"=0xf8, "missile"=0xf4) |
| FUN_002774b0 | OrdnanceBulletClass ReadProperty (Velocity +0xac, effects; §7.3) — shared base for laser/shell |
| FUN_0027cae0 | OrdnanceLaserClass ReadProperty (laser visuals; chains to FUN_002774b0) |
| FUN_0027e668 | OrdnanceShellClass ReadProperty: **Gravity (0xd209ff45) → class+0xdc = value × −9.8** — the ONLY gravity parser in the bolt family |
| FUN_0027c9d0 | OrdnanceLaserClass ctor: LifeSpan default 2.0, Velocity default 150.0 |
| FUN_00277d28 | ordClass vtbl+0x24 GetSpeed = class+0xac (disasm: `lwc1 f0,0xac(a0)`) |
| FUN_00274390 | ordClass vtbl+0x28 GetGravity = **0.0** (base; laser/bullet never override) |
| FUN_00277d30 | ordClass vtbl+0x2c GetMaxRange = Velocity × LifeSpan (`0xac × 0x30`) |
| FUN_00283de8 / FUN_00283f00 | weapon vtbl+0x2c/+0x30 GetOrdnanceSpeed/Gravity — forwards to ordClass vtbl+0x24/+0x28; charge weapons lerp along NextCharge chain (corrects re-aim-assist §5: the lead solver's vcalls land on the WEAPON instance at aimer+0x148, set by FUN_002838d8 → FUN_0026e040) |
| FUN_002762a0 | bolt spawn ctor: velocity = dir × lerp(Velocity, NextCharge.Velocity, charge); collision flag +0x38=1 (once); magnet-target velocity adjust FUN_0036d590 [H on semantics] |
| FUN_002764c0 | ordnance move+collide (dt): predictive ray (mask **0x48**), pos += vel·dt, bounds ±5000 (§5.1) |
| FUN_00272e98 | lifetime: +0x3c −= dt; ≤0 → ExplosionExpire (class+0x34) + die; kill below y=−500 |
| FUN_00276810 | ordnance vtbl+0x3c **impact handler**: deflection check (owner vtbl+0xd0) with LCG jitter re-spawn, damage desc build + delivery, per-surface impact effect (§6.1) |
| FUN_00272f88 | **damage descriptor build**: copy MaxDamage + 8 scales from class+0x4c..+0x6c, charge-lerp toward NextCharge class (§6.1) |
| FUN_001bc190 | **TakeDamage** (health component, server): scale by HealthType, health −= dmg, FF scale DAT_00517508, effects (§6.2) |
| FUN_001bbe00 | TakeDamage client variant (effects only, no health write) |

### BF2 (SWBFRNT2.ELF) — all **[V]** unless noted

| addr | role |
|---|---|
| FUN_00328fb0 | WeaponClass (base) ReadProperty (§7.1) |
| FUN_00330810 | WeaponCannonClass ReadProperty (§7.2) |
| FUN_003304e0 / FUN_00330600 | WeaponCannonClass ctor/copy (defaults ShotsPerSalvo 1 @+0x294, SalvoCount 1 @+0x290, +0x28c=2.0, +0x2e0=1.0); class size 0x300 |
| FUN_003035d0 | RoundsPerClip sub-parser (weaponClass+0x98 component: clip int; <1 → infinite 0x7fffffff) |
| 0x00628338..0x62844c | WeapCannon instance vtable (string @0x628450) |
| ctor before FUN_0032e148 (0x32e0xx) | weapon-instance ctor: spread state −SpreadThreshold (human, fire-point+0xc0 ≥ 0) / 0 (AI) |
| FUN_0032e200 | per-tick: rumble timers, spread recovery (−dt·class+0x27c, floor −class+0x280 / 0) |
| FUN_0032e4b8 | GetYawSpread — identical structure to BF1 incl. AI distance table (0.0997/0.0499/0.0333/0.025/0.02/0.0167/0.0143 × weapon+0x138), fixed term class+0x250 |
| 0x0032e720 | GetPitchSpread (disasm; class+0x24c) |
| 0x0032e748 / 0x0032e750 | set spread multiplier weapon+0x134 / AI multiplier weapon+0x138 (disasm) |
| 0x0032e438 | **SetPostureSpread(weapon, posture, ?, zoomFlag)** (disasm): reads move inputs itself from firePoint+0x6c/+0x70, `base = lerp(Still_or_Zoom, Move, max(|fwd|,|strafe|))` → weapon+0x13c (§4.2) |
| FUN_0032e758 | trigger-press: charge frac +0x120 = pressed/MaxPressedTime(+0x240); **refire +0xc0 = uniform(ShotDelayMin +0x244, ShotDelayMax +0x248)**; salvo counter/timer init; TriggerSingle flag forces input mask |
| FUN_0032e8c0 | **fire cycle** (dt) — BF1 structure + per-salvo `SalvoDelay = uniform(min,max)` via FUN_002a3e48, + wind-up/wind-down timers (class+0x2ac/+0x2b0, weapon+0x140) |
| FUN_0032ef58 | ready-to-fire query (incl. TriggerSingle bit and wind-down maths) |
| FUN_0032edf8 | re-roll refire time = uniform(ShotDelay min,max) |
| FUN_0032f208 | shot spawn — same structure as BF1 (tilt array class+0x2a4/+0x2a8, 2×(rand−rand) LCG on DAT_0073d870, sin/cos dir); also spawns secondary ordnance class+0x2ec (tracer, §7.2 note) |
| FUN_002a3e48 | uniform(min,max) via the inline LCG (`x·0x19660d+0x3c6ef35f`, two steps per 23-bit mantissa) |
| FUN_00310620 | OrdnanceClass (base) ReadProperty — MaxDamage/scales/LifeSpan/falloff keys (§7.3) |
| FUN_00314b98 | OrdnanceBulletClass ReadProperty (Velocity +0xe4) |
| FUN_003204c0 | OrdnanceShellClass ReadProperty: Gravity → +0x11c = value × −9.8 (only gravity parser, same as BF1) |
| FUN_00313600 | ordnance move+collide — same predictive ray, mask **0x90**; pos +0x50, vel +0x110, hit obj +0x134/id +0x138 |
| FUN_00313c40 | impact handler (deflect/effects/damage delivery) [V structure, internals skimmed] |
| FUN_0030fa88 | damage-desc build wrapper: ordClass vtbl(+0x1c)[+0x30](0, charge=ord+0x34, **age=ord+0x40**, out, class, attacker=ord+0x68) |
| FUN_00310178 | **damage descriptor build** = ordClass vtbl+0x30: charge lerp + **time falloff** + team-bonus +25% (§6.1) |
| FUN_001dfb38 | **TakeDamage** (health component at entity+0x170): shield then health, HealthType scale, FF scale DAT_005fcbe0, kill credit +0x190/+0x194 (§6.2) |
| FUN_001df7d0 | TakeDamage client/effects variant |
| GetObjectHealth @0018fb60 / KillObject @0018fcd0 | health component = entity+0x170 (cur/max/add-rate 0x170/0x174/0x178; shield 0x17c/0x180/0x184); last-hit weapon class entity+0x1a4 |

---

## 3. Fire cycle

### 3.1 The loop (BF1 FUN_00288d10; BF2 FUN_0032e8c0 identical shape) [V]

Weapon instance fields (BF1 → BF2): refire accum +0xd0→+0xbc, refire time
+0xd4→+0xc0, salvo counter +0x144→+0x124, salvo timer +0x148→+0x128, charge
fraction +0x140→+0x120, spread state +0x150→+0x130, spread multiplier
+0x154→+0x134, AI-dispersion multiplier +0x158→+0x138, posture base spread
+0x15c→+0x13c, ammo (clip fraction) +0x98→component@+0x90, heat
+0x9c→component@+0x94, salvo-tilt index +0x14c→+0x12c.

```
if fire not held: salvoCounter = 0        // BF2: gated on weapon flag bit 0x22 of +0xb0
while dtLeft > 3.0517578e-05:
  if salvoCounter < 1:                     // waiting for next trigger cycle
    refireAccum += min(dtLeft, refireTime − refireAccum)
    start a salvo iff: refireAccum ≥ refireTime
        AND NOT TriggerSingle             // BF1 class+0xcc==0; BF2 class+0x1e0 bit 0x32==0
        AND vtbl fire-input pred          // BF1 +0xa8 / BF2 +0xdc  (see 3.2)
        AND NOT reloading                 // BF1 +0xb8 / BF2 +0xec
        AND NOT <blocked>                 // BF1 +0xb0 / BF2 +0xe4
    on start: refireAccum −= refireTime   // carries remainder → exact cadence
              salvoCounter = SalvoCount; salvoTimer = InitialSalvoDelay
  else:                                    // inside a salvo
    if salvoTimer ≤ 0:
      repeat ShotsPerSalvo times:
        if ammo < AmmoPerShot: salvoCounter = 0; break        // (clip-fraction units in BF1)
        if firepoint valid AND SpawnShot():                    // §3.3
           ammo −= AmmoPerShot; heat += HeatPerShot
           if not first salvo of cycle and salvoTimer deficit < SalvoDelay: follow-up salvo event
        advance firepoint (multi-barrel ring)                  // BF1 FUN_0026d198 / BF2 FUN_00301d28
      spreadState += spreadMult × SpreadPerShot                // clamped to SpreadLimit
      salvoCounter −= 1
      salvoTimer += SalvoDelay             // BF2: uniform(SalvoDelayMin, SalvoDelayMax)
    consume dtLeft against salvoTimer
per-tick tail: spread recovery + timers (vtbl update)
```

- Refire time: BF1 = ShotDelay (min 0.01, parser-clamped). BF2 = **uniform
  random in [ShotDelayMin, ShotDelayMax]** re-rolled per trigger event
  (FUN_0032e758/FUN_0032edf8); a single ODF value sets min=max (deterministic).
  BF2 also randomizes SalvoDelay per salvo the same way. [V]
- Heat: shot adds HeatPerShot (BF2 component FUN_001e2010 at weapon+0x94;
  overheated bit +8&1 gates firing when HeatPerShot > 0). Heat recovery
  internals (HeatRecoverRate/HeatThreshold consumption) not traced — OPEN.
- Ammo: BF1 stores ammo as a fraction of one clip (AmmoPerShot parsed as
  `value/RoundsPerClip`, ammo state 0..1); RoundsPerClip < 1 → infinite.
  BF2 keeps an int clip component (class+0x98) with the same infinite rule.
  Reload = vtbl "reloading" state driven by ReloadTime (min 0.01); reload
  state machine itself not fully traced — OPEN (offsets known: BF1 class+0x74,
  BF2 class+0xac).

### 3.2 What gates a shot (fire-input predicate, BF1 FUN_00289150) [V]

`cmd = *(char**)(weapon+0x70)` — per-weapon fire command byte written by the
soldier (fire held counter; −1 on release edge):

- MaxPressedTime > 0 (charge weapon): fires when `*cmd == −1` (on RELEASE).
- else TriggerSingle == 0 (automatic): fires while `*cmd > 0` (held).
- else (single): fires only when `*cmd == 1` (fresh press).

BF2 mirrors this with TriggerSingle as class flag +0x1e0 bit 0x32; on trigger
press with that bit set, the input bits are masked (`&0xe00000e0 | 0x1c`) so
only one cycle fires per press (FUN_0032e758). Soldier-side: BF1 fire
processing is skipped entirely in stances 3/5/0xa/0xb (dead/thrown/rolls) —
see re-stance-roll §3.4.

### 3.3 Shot spawn (BF1 FUN_002892a8; BF2 FUN_0032f208 same structure) [V]

1. Build the muzzle frame from the fire-point object (weapon+0x6c): position
   = firePoint+0x90, forward = normalized firePoint+0x50 aim vector.
2. Salvo tilt: if the class has a tilt array (keys 0xa4dfafc0 count /
   0xa4120aa8 pairs, deg→rad), take (pitchTilt, yawTilt) =
   `tilt[weapon+0x14c]`, advance index modulo count (multi-shot patterns,
   e.g. shotguns/award weapons).
3. Spread randomization, per axis (pitch then yaw):
   `angle = tilt + spreadMag × (U₁ − U₂)` — U from the inline LCG
   (`x·0x19660d+0x3c6ef35f`; two LCG steps build one 23-bit-mantissa float in
   [0,1)), so the offset is TRIANGULAR-distributed in ±spreadMag.
   spreadMag(pitch) = vtbl+0x44 = max(state,0) + postureBase + PitchSpread;
   spreadMag(yaw) = vtbl+0x40 = same with YawSpread (radians).
   Global: BF1 DAT_0066eed0, BF2 DAT_0073d870 (same LCG as stance/tumble rolls).
4. Direction: `local = (sin(yaw)·cos(pitch), −sin(pitch), cos(yaw)·cos(pitch))*(decompiler detail omitted)*soldier.vcall294+0x74(KickStrength, ShotDelay × (1/class+0x228 + 2.0))`
   (class+0x228 default 2.0 → duration 2.5×ShotDelay; key hash 0x069ad4d0,
   name uncracked). RecoilLength/Strength Light/Heavy are pad RUMBLE
   parameters (BF2 runs them via FUN_0043d948 timers in FUN_0032e200). [V]
7. BF2 extra: a secondary ordnance class (cannon key 0x6e364fc1 / alias
   0xdddb253a → class+0x2ec, resolved against the ordnance registry
   DAT_00699c4c) is spawned alongside the primary (used for
   chaingun/shotgun TRACER bolts: com_weap_inf_chaingun_tracer). Name
   uncracked; behavior [V], purpose [H].

---

## 4. Spread / accuracy model

### 4.1 State model (both games) [V]

Weapon-instance spread state `s` (radians):

- Init: human player `s = −SpreadThreshold`; AI `s = 0`.
- Per shot: `s += spreadMult × SpreadPerShot`, clamp `s ≤ SpreadLimit`.
- Per tick: `s −= dt × SpreadRecoverRate`, floor `−SpreadThreshold` (human) / 0 (AI).
- Effective spread half-angle per axis at fire time:
  `max(s, 0) + postureBase + {Yaw|Pitch}Spread`, offset drawn triangular ±that.

So SpreadThreshold is a "free shot buffer": the first
`SpreadThreshold/SpreadPerShot` (net of recovery) shots add nothing. All class
values are stored ×0.017453294 (degrees in ODF → radians in engine).
`spreadMult` (weapon+0x154/BF2+0x134) defaults 1.0; setter exists
(vtbl+0x38) — writer not located in the cannon path (likely vehicle/AI
callers) — OPEN.

Legacy aliases folded by the parsers: `MaxSpread v` → PitchSpread = 0.025·v,
YawSpread = 0.05·v (NOT deg→rad!). `SpreadRecover v` → SpreadRecoverRate =
v×SpreadPerShot and SpreadLimit = that/ShotDelay (derived). [V]

### 4.2 Posture/movement/zoom enter here — per-WEAPON keys [V]

Per-posture spread triplets on the WEAPON class (deg→rad), 3 postures × 3:

| posture | Still | Move | Zoom(third) |
|---|---|---|---|
| stand  | StandStillSpread (0x4c081d30) | StandMoveSpread (0xf7561531) | ?0x5bc0137f |
| crouch | CrouchStillSpread (0x81fec758) | CrouchMoveSpread (0xfbde8e79) | ?0x67bfc1d7 |
| prone  | ProneStillSpread (0x4a29a3ca) | ProneMoveSpread (0x73608c73) | ?0x7d9da8bd |

Every tick the soldier calls SetPostureSpread (BF1 FUN_00204748 →
vtbl+0x48 FUN_002889b0; BF2 0x32e438):

```
posture = stance (0 stand / 1 crouch / 2 prone; stance > 2 → 0)
base    = zoomed ? Zoom[posture] : Still[posture]
postureBase = base + (Move[posture] − base) × max(|fwdInput|, |strafeInput|)
```

(BF1 passes moveMag and the zoom flag soldier+0x2c0 — set when zoomed and
ZoomMax > 1 — as args; BF2 reads inputs from firePoint+0x6c/+0x70 itself.)
The "third slot = zoom spread" identification is [V] from the `movz` select
on the zoom flag; the ODF key NAMES for the third slots are uncracked
(all 0.0 on rifles; brute-force against mod-tools vocab failed).

### 4.3 AI accuracy is a hardcoded distance table [V]

For AI shooters (fire-point owner has no player index), IF the class's
YawSpread is 0, the weapon is TriggerSingle with ZoomMax ≥ 6.0
(inclusive — the Imperial scout rifle is exactly 6.0) and the AI has a
current target, the yaw spread getter puts a distance-based dispersion
(radians, step table on the SQUARED distance, no interpolation)
× weapon+0x158 (BF1) / +0x138 (BF2; written every AI aim tick,
re-ai-difficulty §4.2) into the YawSpread slot:
yaw = max(s,0) + postureBase + table×mult. `max(s,0)` is NOT replaced
(re-ai-sniper §2).

| target distance | dispersion |
|---|---|
| < 20 m | 0.0997 |
| < 40 m | 0.0499 |
| < 60 m | 0.0333 |
| < 80 m | 0.025 |
| < 100 m | 0.02 |
| < 120 m | 0.0167 |
| ≥ 120 m | 0.0143 |

(BF1 FUN_002889e8 / BF2 FUN_0032e4b8 — byte-identical constants.) For all
other AI weapons, AI spread uses the same state model as humans but with
floor 0 instead of −SpreadThreshold (no free-shot buffer).

### 4.4 AimFactor* / AimValue: dead data on PS2 — **[V] (strong negative)**

`AimFactorPostureSpecial/Stand/Crouch/Prone`, `AimFactorStrafe`,
`AimFactorMove`, `AimValue` (present in soldier ODFs, values
50/100/140/160/60/70/1.0 — re-stance-roll §) are consumed NOWHERE in either
engine:

- no key strings in either ELF (full printable-string sweep);
- no LE u32 hash constants anywhere in loadable segments;
- no lui/ori- or lui/addiu-composed constants in the code of either binary
  (scan validated by finding ShotDelay/SpreadPerShot the same way).

These keys are PC-engine (AI aim-error) vocabulary that the munge kept; the
PS2 engines' accuracy model is entirely the weapon-side keys above plus the
AI distance table. The port should not wire them into anything.

---

## 5. Ordnance (bolts)

### 5.1 Kinematics + collision (BF1 FUN_002764c0; BF2 FUN_00313600) [V]

> **Corrected 2026-10-22 (card UORD, `re-ordnance-kinds.md` §2):** the predictive ray below tests buildings + terrain
> only (mask 0x48 / BF2 0x90); soldiers, vehicles, riders, water, foliage and shields are struck by the per-frame
> prev -> current sweep (B1 `FUN_00273128`, 0x996 after the predictive ray; BF2 `FUN_0030fad0` 0x132e). A unit that
> steps out of the line IS missed and one that steps in IS hit — the "cannot dodge" caveat below is wrong. Life-out
> spawns `ExplosionExpire` for every class of the family.

- Spawn (BF1 FUN_002762a0): `vel = dir × lerp(Velocity, NextCharge.Velocity,
  chargeFraction)`; remaining-time +0x3c = LifeSpan; collision-pending flag
  +0x38 = 1. If the spawn packet carried a magnet target (aim-assist
  weapon+0x128), FUN_0036d590(target, &vel) adjusts the velocity [H exact
  math — likely aims at the stored aim point; trace me].
- First update after spawn: ONE raycast from position along normalized
  velocity, **length = |vel| × remainingTime** (the entire future path),
  collision mask BF1 **0x48** / BF2 **0x90**. On hit: remainingTime ×=
  hitFraction; remember hit prim/object/object-id.
- Every tick: `pos += vel × min(dt, remainingTime)`; age += dt;
  remainingTime −= dt. There is NO further collision testing and NO gravity
  for the bolt/laser/bullet classes (gravity getter is the base-class
  constant 0.0 — Gravity ODF keys on bolts, e.g. rifle `Gravity = 1.0`, are
  parsed only by the `shell` class where value × −9.8 applies). World bounds
  kill at ±5000 (x/z), y < −500.
- When remainingTime hits 0: if a hit object was recorded and its id still
  matches (+0xb4 BF1 / +0x230 BF2 guard) → impact handler; if the object died
  in flight → bolt dies with no impact; if no hit was recorded → expire:
  spawn ExplosionExpire class at final position.
- **Caveat for the port**: because collision is one predictive ray at spawn,
  (a) targets CANNOT dodge a bolt after it is fired (the hit lands at the
  precomputed time as long as the entity is alive), and (b) anything moving
  INTO the path after spawn is never hit. This is authentic PS2 behavior
  [V for the code path; no re-arm of the +0x38 flag exists outside spawn
  ctors — grep-verified]. Confirm on PCSX2 before relying on it (an external
  mover system re-arming the flag would be invisible to this analysis) [H].
- Max range = Velocity × LifeSpan (ordClass vtbl+0x2c FUN_00277d30); the
  weapon caches it at weaponClass+0xd0 when the Ordnance key resolves
  (overridable by ODF key 0x8474ea2c; kept ≥ LockOnRange).

### 5.2 Deflection (lightsabers) — BF1 FUN_00276810 [V]

Before damage, the impact handler asks the victim: `owner.vcall+0xd0(dirN,
hitPos)`. If it accepts (saber block): the bolt direction is reflected with
±0.5 uniform jitter on two axes (LCG), a fresh ordnance spawn packet is built
(owner becomes the deflector — `vcall+0x2c` new team/owner) and re-spawned via
ordnance class vtbl+0x10; the function returns "still alive". Damage is
skipped entirely.

---

## 6. Damage

### 6.1 Damage descriptor (built per impact from the ordnance class)

**BF1 FUN_00272f88** [V]: copies from OrdnanceClass:
`desc[4] = MaxDamage (+0x4c)`, `desc[5..11] = PersonScale(+0x50),
AnimalScale(+0x54), DroidScale(+0x58), VehicleScale(+0x5c),
BuildingScale(+0x60), bldg2(+0x64), bldg3(+0x68)`, `desc[12] =
ShieldScale(+0x6c)`; desc[0..3] = attacker info from ordnance+0x58/+0x60.
If chargeFraction > 0, every value lerps toward the NextCharge class
(class+0x2c) values. **No distance/time falloff in BF1.**

**BF2 FUN_00310178** (= ordClass vtbl+0x30, called by FUN_0030fa88 with
argument 2 = charge, argument 3 = **bolt age in seconds** (ord+0x40)) [V]:

```
desc = class+0x54..+0x94 (17 floats; damage = class+0x70, scales +0x74..+0x90,
                          shield +0x94)
charge lerp toward NextCharge (+0x2c) as BF1
t = age − class+0x34                     // falloff start TIME (rifle: 0.4)
if t > 0:
    frac = class+0x38 > 0 ? clamp(t / class+0x38, 0, 1) : 1   // fade duration (0.2)
    end  = class+0x3c                    // end damage (rifle: 50); < 0 → no falloff
    if end < 0: end = damage
    damage += (end − damage) × frac
if attackerTeam has active bonus hash 0xbb03442e:              // ActivateBonus registry 0x69a248
    damage += damage × class+0x40        // rifle: +25% (0x97481656 = 0.25)
```

So BF2 blaster falloff is TIME-based (equivalently distance = t×Velocity
since bolts don't decelerate): rifle 80 dmg to 64 m, linear to 50 by 96 m,
50 out to the 128 m max range. Key names for +0x34/+0x38/+0x3c/+0x40
(hashes 0xa74561b2 / 0xee08d1ba / 0x955f71e9 / 0x97481656) are uncracked —
functionally: FalloffStartTime / FalloffDuration / EndDamage /
TeamBonusDamageScale. The bonus name behind 0xbb03442e is uncracked (likely
the galactic-conquest "blaster amplification" card; that Lua string itself
doesn't hash to it — OPEN).

### 6.2 Damage application (health component)

> **Corrected 2026-10-27 (issue #41, `re-hitzones.md`):** soldiers DO have a per-node multiplier: every soldier
> class with no ODF `HitLocation` (all of them) gets a hard-coded default {head prim, x3.0} - BF2 `FUN_002517e8`
> (B2:170444-170450, node 0 = coll_head), BF1 `FUN_0020e2e0` (B1:122725, node 2). Droidekas (walkerdroid) and the
> acklay get none. The soldier's TakeDamage is BF2 `FUN_00238e20` (its health vtable `DAT_0061ea80`, slot +0x24 ->
> `FUN_0024ec88`), which this section's chain missed; `headShots` / the "headshot" effect count these soldier hits.


**BF2 FUN_001dfb38** (server; client variant FUN_001df7d0) [V]. Health
component = entity+0x170: `[0]` health, `[1]` max, `[3]` shield, `[4]` shield
max, `[7]` regen/bonus fraction (cf. "player%d.bonusHealthFraction"),
`[0x2e]` flags word — **low 3 bits = HealthType** (person/animal/droid/
vehicle/building/...), bit 3 = "is important/local", bit 5 = has shield;
`[0x34]`(BF1)/`[0x3e]`(BF2) = team.

```
dmg = desc.damage × param1 × desc.scale2      // param1 = caller multiplier (1.0 direct hit,
                                              //   radius fraction for explosions [H]; answered:
                                              //   re-vehicle-death.md §3 — explosions pass 1.0 here and
                                              //   scale desc.damage by the inner/outer falloff first)
if victimTeam == attackerTeam: dmg ×= friendlyFireScale   // BF1 DAT_00517508 / BF2 DAT_005fcbe0
                                              // (players sometimes exempted via checks)
shield phase (flag bit): shield −= dmg × desc.shieldScale; remainder carries
health phase: dmg ×= desc.scale[HealthType & 7]
              health −= dmg
              owner.vcall(+0x24/+0x28)(dmg)   // on-damaged → death handling when ≤ 0
              attribution: victim+0x190 = attacker weapon-class, +0x194 += dmg
                           (feeds GetObjectLastHitWeaponClass, kill credit)
              local-player feedback: camera shake if dmg > 2% max health,
              red screen flash alpha 32..128 (both games, same constants)
```

BF1 FUN_001bc190 is the same minus attribution fields (desc[4] damage,
desc[5+type] scales, desc[12] shield). **There are no per-bodypart or
headshot multipliers anywhere in the chain** — damage varies only by ordnance
scales × victim HealthType (and BF2's time falloff). [V]

The impact handler reaches TakeDamage through the hit prim's owner interface
(`prim.iface+0x1c*(decompiler detail omitted)*engine/artifacts/lead_tmp/ffln/` (`ff_scale.py`, `tk_ps2.py`, `tk_report.py`, `run_tk.sh`,
`ps2_bf2_kam1_playtest.jsonl`). Checks run: `re_check.py fn b2 91318 91324 90935 90953 359400 359406 346223 346227`,
`fn b1 64020 64038`, `writers b2 DAT_005fcbe0` (1: FUN_001df3c8), `writers b1 DAT_00517508` (1: FUN_001bbaf8),
`word b2 0x5fcbe0` / `word b1 0x517508` (both 1.0), `callers` of both setters.

- **The rule** [V] (section 6.2 and re-vehicle-death.md section 4): B2 `FUN_001dfb38` 91318-91326 — the victim's team
  is friendly to the attacker's: when the attacker is not a human (`attacker+0x130 < 0`) and the victim is
  (`controller+0xc0 >= 0`) the function RETURNS (no damage, no shield); otherwise `dmg *= DAT_005fcbe0`. The gate is
  the team RELATION byte > 0 (a team is friendly to itself; on Endor team 1 and the locals' team 3 are friends); the
  client twin `FUN_001df7d0` (91177) gates on team equality. B1 `FUN_001bc190` 64355-64362 (and its sibling
  `FUN_001bbe00`, 64222) the same with `DAT_00517508`, gated on team EQUALITY (attacker `+0x9c < 0`, victim
  controller `+0xe8 >= 0`). So: bot -> bot friend and human -> any friend take the scale; bot -> human friend takes
  nothing.
- **The scale's writers** [V]: one setter each (B2 `FUN_001df3c8` 90953, B1 `FUN_001bbaf8` 64038). `FUN_001df328`
  (90928-90943; B1 `FUN_001bba58` 64013-64028) feeds it the profile's percent byte x 0.01 (B2 +0x202, B1 +0x15f) when
  a profile exists and the flag `DAT_00600548` / `DAT_00600549` (B1 `DAT_00518be4` / `DAT_00518be8`) is 0, else 1.0 or
  0.0 by `DAT_00600506` / `DAT_00518bd8` (written by `ScriptCB_SetNetGameDefaults`: the netplay option [D]). Other
  callers of the setter: B2 `FUN_00435868` (493431: a byte at +0x1d2 x 0.01), `FUN_00323178` (315992 / 316062: 1.0
  for the length of the call, then the old value back); B1 `FUN_0034cf08` (337067: a byte at +0x133 x 0.01).
- **The scale in a running battle** [M]: `ff_scale.py` reads the global out of every battle state of the reference
  library: **0.99999994 in 150 of 150 BF1 states and 176 of 176 BF2 states** — the byte is 100 (100 x 0.01 under the
  EE's multiply). Instant action runs friendly fire at full damage in both games. No state holds another value; an
  options screen that sets the byte was not looked for [H].
- **The PS2's bots kill their own team** [M]. BF2 keeps per player a statistics block, `*(0x661394) + i * 0x210`
  (`FUN_00350be8`); the death handler `FUN_0034f650` sends a friendly victim (the killer himself included) to
  `FUN_00362c60` (346227), which adds 1 to the killer's kill byte (+4 + team slot x 2 + hero) AND to his team-kill
  byte (+8 + ...) (359400-359404) and to a per-victim byte through the pointer at +0x14 (359405-359406). `tk_ps2.py`
  polls those from a live battle. BF2 KAM1 conquest, 16 AI a team, the player spawned, pinned at the maintainer's spot
  (-4, 60.3, 108.6) and kept alive, 515 s of wall time (`run_tk.sh bf2_kam1 _playtest`): **91 kills by AI, of them 11 of
  a teammate (12.1 %; 12.6 % of the 87 that were not the killer's own) and 4 of the killer himself**; the human
  killed nobody. The per-victim bytes agree with the counters for every player (enemy 76 + friend 11 + self 4 = 91;
  the enemy path `FUN_00362268` 359237-359243 writes the same kill byte and table). These are CREDITED kills, by any
  cause (a grenade's blast as well as a bolt), and all 11 are AI on AI (the rule above spares the human). One
  battle, one map: the share is a reading, not a distribution.
- Verifier (one foreground Opus agent, 10 claims, citations only): 8 SUPPORTED, 2 PARTLY (B1's gate is team equality
  and its line is 64361, not 64222's function; the setters have the further callers listed above) — folded in.
- So the PS2 soldier AI has no working line-of-fire rule against its own team [D from the 12 %]. Card SAIR read the
  fire decision (`re-soldier-ai.md` 4.9, 6): it does test for a friend, but only a HUMAN one - the veto B2
  `FUN_002bb900` / B1 `FUN_0023cfc8` holds fire while a human friend within 50 m is on the aim ray; an AI team mate
  is never in that list [V, verifier-checked].
- **Port**: `Sim::friendly_fire_scale` 1.0 was already the PS2's; `resolve_bolt` now drops a bot's bolt into a human
  of its team (a blast did already). Before the port's own rule, its team-kill share on the same map was the PS2's:
  12.2 % (8 seeds), 17.7 % over the 32 maps. The line-of-fire rule (`core::ai::brain::friend_in_line`) is OURS, a
  named deviation: the maintainer's "the bots have no sense of teammates".

---

## 7. ODF key / hash / offset tables

### 7.1 Base WeaponClass (BF1 FUN_002864f0 / BF2 FUN_00328fb0)

| key | hash | BF1 off | BF2 off | notes |
|---|---|---|---|---|
| Ordnance / OrdnanceName | 0x399bf05d / 0xf8899c5e | +0x5c | +0x88 [H off] | class ptr via registry (BF1 DAT_00588f5c / BF2 DAT_00699b84-family); BF1 also caches maxRange→+0xd0 |
| RoundsPerClip | 0x7bff9ec9 | +0x60 (int; <1→∞ 0x7fffffff) | component @+0x98 (FUN_003035d0) | |
| AmmoPerShot | 0x222090f6 | +0x64 = value/RoundsPerClip | +0x8c | BF1 ammo state is clip-fraction |
| HeatPerShot | 0x67cffd1a | +0x68 | +0x90 | |
| HeatRecoverRate | 0x57e7bba3 | +0x6c | +0x94 | |
| HeatThreshold | 0x5621511e | +0x70 | ? | |
| ReloadTime | 0x9fc1a1ad (alias 0x136a7450) | +0x74 (min 0.01) | +0xac (min 0.01) | |
| ZoomMin / ZoomMax / ZoomRate | 0x1881b1a6 / 0x2a96e8b4 / 0x9573fb20 | +0x78/+0x7c/+0x80 | +0xb0/+0xb4/+0xb8 | 0→1.0 coerced; ZoomMax is the only zoom key the AI table reads; ctor default 1.0 (BF2 FUN_00327e98 :319499, BF1 FUN_00285a98 :209461) |
| ZoomFirstPerson | 0x14ec7e1d | +0x8c bool | +0x1e0 bit 0x33 | |
| TriggerSingle | 0xed9d56db | **+0xcc** bool | **+0x1e0 bit 0x32** | single-shot gate; +sniper-AI-table gate |
| RecoilStrengthHeavy / RecoilDecayLight / RecoilLengthLight / RecoilLengthHeavy / RecoilStrengthLight / RecoilDecayHeavy | 0x12ba9f01 / 0xa9b93c09 / 0x903574d1 / 0x4b92a06a / 0xcf3649e6 / 0xb97f1f52 | +0x90/+0xa0-ish region | +0xc4/+0xdc/+0xcc/+0xd0/+0xc8/+0xe0 | pad rumble |
| LockOnRange | 0x7f012f70 | +0xd4 | +0x104 | aim assist (re-aim-assist) |
| (max-range override) | 0x8474ea2c | +0xd0 | +0x100 | kept ≥ LockOnRange; auto-set from ordnance |
| AutoAimSize / LockOnAngle / LockOffAngle | 0x7dbe88fa / 0x3935c7ca / 0xc915b156 | +0xd8/+0xd8/+0xdc | +0x108/+0x108/+0x10c | ×0.072 / deg→rad |
| MinRange / OptimalRange / MaxRange | 0x1ef25b6c / 0x778f9aae / 0x43bcebe6 | +0xe4/+0xe8/+0xec | +0x110/+0x114/+0x118 | AI range prefs |
| Target* flags | (re-aim-assist §4) | +0xf4/+0xf8 | +0x11c/+0x120 | AI target filters |
| FirePointName | 0x4274fc96 | +0x4c geo + hp lookup | +0x74 | default "hp_fire" |
| FireSound etc. | (various) | ≥+0x150 | ≥+0x128 | names in §example dumps |

### 7.2 WeaponCannonClass (BF1 FUN_00289b48 / BF2 FUN_00330810)

| key | hash | BF1 off | BF2 off | transform |
|---|---|---|---|---|
| MaxPressedTime | 0x2819ffa0 | +0x1e0 | +0x240 | charge weapons: fire on release, fraction = pressed/this |
| ShotDelay | 0xd9356908 | +0x1e4 (single) | **+0x244/+0x248 min/max pair** | min clamped ≥ 0.01; 1 value → both |
| PitchSpread | 0x8d0cb6ca | +0x1e8 | +0x24c | deg→rad |
| YawSpread | 0xebefdcd7 | +0x1ec | +0x250 | deg→rad |
| MaxSpread (legacy) | 0xaedcbd12 | +0x1e8=v·0.025, +0x1ec=v·0.05 | +0x24c/+0x250 same | |
| Stand/Crouch/Prone Still/Move/Zoom spreads | see §4.2 | +0x1f0..+0x210 (3×3 floats) | +0x254..+0x274 | deg→rad |
| SpreadPerShot | 0x43ab2c7d (alias 0xef4935b0) | +0x214 | +0x278 | deg→rad |
| SpreadRecoverRate | 0xccedb638 | +0x218 | +0x27c | deg→rad, per second |
| SpreadRecover (legacy) | 0x33841a8a | +0x218=v×PerShot; +0x220 derived | +0x27c/+0x284 same | |
| SpreadThreshold | 0x2613b9b9 | +0x21c | +0x280 | deg→rad (negative buffer) |
| SpreadLimit | 0x85a6b6fd | +0x220 | +0x284 | deg→rad |
| KickStrength | 0x16e4db4a | +0x224 | +0x288 | deg→rad; view kick |
| (kick-duration divisor) | 0x069ad4d0 | +0x228 (default 2.0) | +0x28c | duration = ShotDelay×(1/v+2); name uncracked |
| SalvoCount | 0x9260ea63 | +0x22c int | +0x290 | trigger cycles per press... salvos per trigger |
| ShotsPerSalvo | 0x03b38558 | +0x230 int | +0x294 | default 1 |
| SalvoDelay | 0xd76f37db | +0x234 (single) | **+0x298/+0x29c min/max** | |
| InitialSalvoDelay | 0xc8388d9d | +0x238 | +0x2a0 | |
| (salvo tilt count / entries) | 0xa4dfafc0 / 0xa4120aa8 | +0x23c ptr,+0x240 n | +0x2a4/+0x2a8 | pairs deg→rad, cycled per shot |
| (BF2 only: wind-up? / wind-down?) | 0x44edeee4 / 0x9ecac3a1 | — | +0x2ac / +0x2b0 | pre/post-fire timers in fire cycle; names uncracked |
| (BF2 only: secondary/tracer ordnance) | 0x6e364fc1 / 0xdddb253a | — | +0x2ec class ptr | spawned per shot alongside primary |
| (BF2 only, unread) | 0x7236488b, 0x1a2034be, 0x0f1ca143, 0x25820dbf, 0x22e5153c, 0x82e86218, 0x4225631d, 0x01a333e4, ... | — | +0x2e4,+0x2bc,+0x2d4,+0x2c4,+0x2c0,+0x2f0,... | uncracked names, rifle values 0 |

Runtime instance offsets: §3.1. LockTime-like key 0x448fbce3 (BF1 wpnc dump
value 0.4) is in the BASE class region — untraced.

### 7.3 OrdnanceClass family (BF1 FUN_002738a8+FUN_002774b0(+FUN_0027e668) / BF2 FUN_00310620+FUN_00314b98(+FUN_003204c0))

> **Corrected 2026-10-22 (card UORD, `re-ordnance-kinds.md` §6):** the shell setter (`FUN_0027e668` / `FUN_003204c0`)
> reads only `Gravity`; `Rebound` / `Friction` are read by the `grenade` ordnance class and items, never by a shell.

| key | hash | BF1 off | BF2 off | notes |
|---|---|---|---|---|
| LifeSpan | 0x7c7c544b | +0x30 | +0x30 | seconds; maxRange = Velocity×LifeSpan |
| Velocity | 0x32741c32 | +0xac (bullet/laser) | +0xe4 | m/s along spawn dir |
| Gravity | 0xd209ff45 | **shell only**: +0xdc = v×−9.8 | **shell only**: +0x11c = v×−9.8 | bolts/lasers/bullets: gravity ≡ 0 (base getter FUN_00274390); rifle bolts fly straight, their `Gravity=1.0` key is dead |
| Rebound | 0x678de338 | shell +0xe0 | shell family | bounce factor (shells) |
| MaxDamage / Damage | 0x47d0e794 / 0x59e94c40 | +0x4c | +0x70 | |
| PersonScale | 0x0bb94602 | +0x50 | +0x74 | HealthType 0 |
| AnimalScale | 0x9aa17a49 | +0x54 | +0x78 | type 1 |
| DroidScale | 0x338a07cd | +0x58 | +0x7c | type 2 |
| VehicleScale | 0x6ccdc5ef | +0x5c | +0x80 | type 3 |
| BuildingScale | 0x02168f61 | +0x60(+0x64,+0x68) | +0x84/+0x88/+0x8c | type 4 (+2 building variants) |
| ShieldScale | 0xd0545c72 | +0x6c | +0x94 | shield phase |
| HealthScale (person+animal+droid) | 0xc37970cf | +0x50/54/58 | +0x74/78/7c | legacy combo |
| (mech combo) | 0xa0bb60ce | +0x5c..+0x68 | +0x80..+0x8c | vehicle+buildings |
| **BF2 falloff start time** | 0xa74561b2 | — | +0x34 | rifle 0.4 s; name uncracked |
| **BF2 falloff duration** | 0xee08d1ba | — | +0x38 | rifle 0.2 s |
| **BF2 end damage** | 0x955f71e9 | — | +0x3c | rifle 50.0; <0 disables |
| **BF2 team-bonus damage scale** | 0x97481656 | — | +0x40 | rifle 0.25 (+25% with bonus 0xbb03442e) |
| NextCharge | 0x8316f444 | +0x2c chain | +0x2c | charge-lerp target class (award/charge weapons) |
| Explosion / ExplosionImpact | 0x02bb1fe0 / 0x61a8bb12 (0x9b30e763) | +0x38 | +0x4c | impact explosion class |
| ExplosionExpire | 0x2e51081f | +0x34 | +0x48 | expiry explosion |
| ImpactEffectStatic/Terrain/Water/Shield | 0x087ea2f0 / 0x5a554273 / 0xecaaa59d / 0x4bc6c22f | +0xb8/+0xbc/+0xc0/+0xc4 | +0xf0/+0xf4/... | per-surface effects; two more (0xd6220fe6/0xa3091269 → +0xb0/+0xb4) uncracked |
| LaserTexture/LaserLength/LaserWidth/GlowLength etc. | (see dumps) | laser cls +0xdc.. | +0x100.. | visuals; LaserTexture has named presets (redlaser_L/greenlaser_L/bluelaser_L) with hardcoded width/length |
| TrailEffect | 0x7e71e2c6 | +0x70 | +0x98 | |
| OrdnanceSound / CollisionSound | 0xccf9be5c / 0xc28f0c96 | +0x74/+0x7c | +0x9c/+0xa4 | |

Class defaults (OrdnanceLaser ctor FUN_0027c9d0): LifeSpan 2.0, Velocity 150.0.

---

## 8. Worked examples (byte-verified from SIDE LVL wpnc/ordc dumps)

### 8.1 BF1 `all_weap_inf_rifle` (SIDE/ALL.LVL, base `cannon`) + `all_weap_inf_rifle_ord` (base `bolt`)

| parameter | value | derived behavior |
|---|---|---|
| ShotDelay | 0.2 | 5 shots/s, automatic (TriggerSingle 0) |
| SalvoCount / ShotsPerSalvo / SalvoDelay | 1 / (1) / 0.0 | plain auto rifle |
| RoundsPerClip / ReloadTime | 50 / 1.5 | 10 s sustained; AmmoPerShot default → 0.02 clip-fraction/shot; WeaponAmmo 6 (soldier prop) = total clips including the loaded one; see re-ammo-reload.md |
| Yaw/PitchSpread | 0.25° | constant dispersion floor |
| SpreadPerShot / RecoverRate / Threshold / Limit | 1.1° / 4.8°/s / 1.6° / 6.0° | net growth while firing = 1.1 − 4.8×0.2 = +0.14°/shot from −1.6° → bloom only begins ~shot 13, caps at 6° |
| posture spreads | all 0.0 | NO movement/stance accuracy penalty on the rifle |
| Ordnance Velocity / LifeSpan | 80 / 1.5 | straight-line bolt, max range 120 m, 0.25 s to 20 m |
| Gravity | 1.0 (DEAD — bolt class ignores it) | flat trajectory |
| MaxDamage | 60, no falloff | vs 300 HP trooper: 5 shots (1.0 s) at any range |
| scales | Person/Droid/Animal 1.0, Vehicle 0.0, Building 0.1, Shield 1.0 | rifles cannot damage vehicles at all |
| LockOnRange | 64 | aim-assist arm range |
| Recoil (rumble) | L 0.1/1.0, H 0.1/0.2 | pad rumble only; KickStrength absent → no view kick |

### 8.2 BF2 `rep_weap_inf_rifle` → `com_weap_inf_rifle` (SIDE/REP.LVL) + `com_weap_inf_rifle_ord` (base `bolt`)

Note BF2 weapon classes use wpnc→wpnc BASE inheritance (rep skin over com
stats), like the entc chain in RE_LOG §BF2 soldier defaults.

| parameter | value | derived behavior |
|---|---|---|
| ShotDelay | 0.2 (min=max) | 5 shots/s auto (TriggerSingle 0); BF2 would randomize if a pair were given |
| SalvoCount / SalvoDelay / InitialSalvoDelay | 1 / 0.0 / 0.0 | |
| RoundsPerClip / ReloadTime | 50 / 1.5 | |
| Yaw/PitchSpread | 0.25° | |
| SpreadPerShot / RecoverRate / Threshold / Limit | 1.2° / 4.8°/s / 1.6° / 5.0° | net +0.24°/shot from −1.6° → bloom begins ~shot 8, caps 5° |
| posture spreads | all 0.0 | |
| Ordnance Velocity / LifeSpan | 160 / 0.8 | max range 128 m, half the flight time of BF1 |
| MaxDamage / falloff | 80; start 0.4 s, fade 0.2 s, end 50 | full 80 to 64 m, linear to 50 at 96 m, 50 to 128 m. dmg(d)=80−30·(d−64)/32 on [64,96] |
| team bonus | +25% (0x97481656=0.25) with bonus 0xbb03442e active | 100 close-range with the card |
| scales | Person/Droid/Animal 1.0, Vehicle 0.1, Building 0.1, Shield 1.0 | BF2 rifles chip vehicles (8/6.5/5 per hit) |
| vs 400 HP trooper | 5 shots ≤64 m; 6–7 mid; 8 at ≥96 m | range now matters |
| LockOnRange | 40 | (BF1 rifle was 64) |
| MinRange/Optimal/Max | 0/24/64 | AI ranges (BF1: 0/16/48) |

### 8.3 BF1 vs BF2 rifle diff summary

| aspect | BF1 | BF2 |
|---|---|---|
| refire | fixed ShotDelay | uniform [min,max] (equal for rifle) |
| damage | 60 flat | 80→50 time-falloff + optional +25% team bonus |
| bolt | 80 m/s, 1.5 s | 160 m/s, 0.8 s |
| vehicle damage | 0.0 | 0.1 |
| spread | −1.6 buffer, +1.1/shot, cap 6° | −1.6, +1.2/shot, cap 5° |
| trooper TTK | 5×60 vs 300 HP | 5–8×(80..50) vs 400 HP |

---

## 9. Reference test vectors (hand-computable, → Tier-1 tests)

All angles in degrees for readability (engine: ×0.017453294), dt = 1/30.

1. **Spread bloom (BF1 rifle, standing still, held trigger)**: state sampled
   at each shot (fire at t = 0.0, 0.2, 0.4 ...):
   s₁ = −1.6, s₂ = −1.46, s₃ = −1.32, ... sₙ = −1.6 + 0.14(n−1);
   dispersion half-angle per axis = max(sₙ,0) + 0.25;
   first widened shot n=13 (s=0.08 → 0.33°); cap when s reaches 6.0
   (dispersion 6.25°) around shot 55. Recovery to −1.6 after ceasing fire
   from cap: 7.6/4.8 = 1.583 s.
2. **BF2 spread bloom**: sₙ = −1.6 + 0.24(n−1); first widened shot n=8;
   cap 5.0 (dispersion 5.25°) around shot 29.
3. **Refire cadence**: shots at exactly k×0.2 s (accumulator carries
   remainder, so cadence is drift-free at any frame rate); 2 shots in one
   update if dt ≥ 0.4 s (clamped by the 0.1 s frame clamp → max 1 catch-up).
4. **BF2 damage vs distance** (rifle): d≤64 → 80; d=72 → 72.5; d=80 → 65;
   d=88 → 57.5; d≥96 → 50; d>128 → bolt expired. (d = 160·age.)
5. **Bolt flight**: BF1 rifle bolt at 20 m target: impact at t=0.25 s, no
   drop. Ray at spawn length = 80×1.5 = 120 m (mask 0x48).
6. **TTK BF1**: rifle vs all_inf_soldier (300 HP, HealthType person, scale
   1.0): 5 shots = 0.8 s from first shot. Vs building-type: 60×0.1=6/shot.
7. **Triangular dispersion RNG**: with LCG state X (BF1 DAT_0066eed0 / BF2
   DAT_0073d870), each axis draws U₁,U₂ (two LCG steps each:
   `frac(x₁,x₂) = float(((x₁>>16 | x₂&0xffff0000)>>9)|0x3f800000) − 1.0`),
   offset = mag×(U₁−U₂). Seedable and bit-reproducible in the port.
8. **Overheat-free**: rifle HeatPerShot unset → heat gate never trips
   (BF2 gate requires class+0x90 > 0).

---

## 10. Open questions

1. **Reload state machine** — where ReloadTime is consumed, reload
   start/interrupt rules, and BF1's clip-refill write (weapon vtbl+0xb8-gated
   region; BF2 vtbl+0xec). Offsets known; trace before porting reload feel.
2. **Heat component internals** (BF2 FUN_001e2010 owner) — HeatRecoverRate/
   HeatThreshold application, overheat penalty timing.
3. **Uncracked key names** (functionally understood): BF2 falloff trio +
   bonus scale (0xa74561b2/0xee08d1ba/0x955f71e9/0x97481656); zoom-spread
   slots (0x5bc0137f/0x67bfc1d7/0x7d9da8bd); kick divisor 0x069ad4d0;
   BF2 wind-up/down (0x44edeee4/0x9ecac3a1); tracer ordnance
   (0x6e364fc1/0xdddb253a); salvo tilt pair (0xa4dfafc0/0xa4120aa8);
   two impact effects (0xd6220fe6/0xa3091269); bonus name 0xbb03442e.
   Brute-force against the PC mod-tools ODF corpus (actual documentation
   files, not just my vocabulary list) when available.
4. **Spread multiplier writer** (weapon+0x154/+0x134) and the AI dispersion
   multiplier writer (+0x158/+0x138) — expected AI-difficulty callers.
5. **Magnet-target velocity adjust at spawn** (BF1 FUN_0036d590) — quantify
   (is fired-bolt "magnetism" real, and how strong?). PCSX2: fire with/without
   a magnet target and dump the spawn velocity.
6. **Predictive-raycast caveat** (§5.1): confirm on PCSX2 that bolts truly
   ignore objects entering the path post-spawn (walk a droid into a slow
   in-flight bolt's path).
7. **Collision masks** 0x48 (BF1) / 0x90 (BF2) layer semantics — same trace
   as re-aim-assist mask 0x5e.
8. **BF2 GetObjectHealth fields** +0x174/+0x178 exact meaning (max +
   AddHealth rate assumed) and death threshold handling in the owner
   on-damaged vcall.
9. argument 1 (damage multiplier) = 1.0 for direct bolt hits — assumed, verify.

## 11. Port notes

- Implement the fire cycle exactly as §3.1 (dt-chunk loop, remainder-carrying
  accumulator) — it is frame-rate independent by construction and matches our
  fixed 30 Hz tick trivially.
- Spread: one signed state var per weapon in radians; the −Threshold floor
  for humans is the load-bearing quirk (free early shots). Cite
  FUN_00288d10/FUN_00288930/FUN_002889b0 (BF1), FUN_0032e8c0/FUN_0032e200/
  0x32e438 (BF2).
- Do NOT wire AimFactor*/AimValue into accuracy (§4.4) — spread is
  weapon-side only; keep the soldier keys parsed-but-unused for data parity.
- Bolts: straight-line, Velocity×LifeSpan range, single predictive raycast
  is authentic; for openbf decide deliberately whether to keep the
  "no late-entry hits" behavior (flag it if we choose per-tick sweeps).
- BF2 damage falloff runs on bolt AGE, not distance — keep it that way so
  charge/NextCharge speed lerps interact identically.
- Both games' gameplay RNG here is the inline LCG (0x19660d/0x3c6ef35f),
  NOT the SCE rand() from RE_LOG §RNG — the port's SimRng needs both.
