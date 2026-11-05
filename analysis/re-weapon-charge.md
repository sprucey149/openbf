# RE: charge weapons (BF1 SWBFRONT.ELF / BF2 SWBFRNT2.ELF)

Tags: [V] read in the decomp / ELF at the cited function and line; [D] deduced from [V] pieces; [H] hypothesis.
Line numbers: B1 = analysis/SWBFRONT.ELF-decomp.c, B2 = analysis/SWBFRNT2.ELF-decomp.c. Ghidra prints the
negative hashes sign-extended (`0xffffffff8316f444`); both forms and the ELF lui/ori builds were searched (§0).

## Headline

- **Two independent NextCharge chains.** `NextCharge` in a WEAPON class and `NextCharge` in an ORDNANCE class
  both mean "clone the class as it stands, store this threshold in the clone, and send every later key of the
  ODF to the clone". The weapon chain is a list of discrete **charge stages**; the ordnance chain is a list of
  **lerp knots**. [V]
- **Weapon stages (discrete).** While the trigger is held, the weapon's *current class* (weapon+0x64) steps
  forward along the chain while `pressed / MaxPressedTime > next stage's NextCharge`. At release the shot is
  built from the current class: its `OrdnanceName`, `ShotsPerSalvo`, the per-shot offset array, `HeatPerShot`,
  everything. Back to idle (and at the next charge start) the current class resets to the root.
  Since a stage is a clone, it keeps every key written before its `NextCharge` line; it changes only the keys
  authored after it. [V]
- **Ordnance knots (continuous).** At spawn, the ordnance instance picks the segment [t_i, t_i+1] that holds
  the charge and stores `s = (charge - t_i) / (t_i+1 - t_i)` (0 on the last knot). Then velocity, LifeSpan,
  damage + every damage scale, shell gravity and missile turn/waver lerp between knot i and knot i+1 by `s`. [V]
- **charge** = pressed time / MaxPressedTime, clamped to 1 by the cannon's accumulator (1.0 when
  MaxPressedTime <= 0). The weapon passes it in the spawn packet (+0x20). The emitter (arc caster) reads it raw:
  damage = lerp(Damage, MaxDamage, charge). [V]
- **Light / Heavy are the two DualShock rumble motors, not fire modes.** `ChargeDelay*` = delay before the motor
  ramps, `ChargeRate*` = ramp rate per second, `MaxChargeStrength*` = cap. Light = small motor (on/off),
  Heavy = large motor (0-255, x0.7). Only for a local human player. They have no gameplay effect. [V for the
  code chain; D for the motor semantics]
- On the discs, the weapon stages are what make the mortar (tap = shell, hold > 0.9 = bouncing sticky, both
  games), the bowcaster spread (BF1: 1 -> 3 -> 5 -> 7 bolts; BF2 the other way round: 7-bolt spread -> one
  heavy bolt) and the BF1 arc caster's heat steps. Ordnance knots include BF2's Hoth dish turret
  (Damage 1000 -> 3000), BF1's Gungan haywire (Velocity 1.5 -> 25), and BF1 Ewok rock
  (shell, Velocity 5 -> 20) and spear (sticky, Velocity 5 -> 25). [V data]

## 0. Search receipts

- Decomp text, both corpora, all forms (hex, `-0x`, sign-extended `0xffffffff…`, decimal):
  `scripts/tools/charge_keys_decomp.py`. ELF scan, both ELFs, every PT_LOAD word, raw data words plus lui followed within 12
  instructions by ori/addiu on the same register: `scripts/tools/charge_keys_elf.py`. Neither found a key in a data word.
  The ELF builds match the decomp hits one for one:

| key | B1 hits (fn, line) | B2 hits (fn, line) |
|---|---|---|
| MaxPressedTime 0x2819ffa0 | cannon `FUN_00289b48` 212000 (+0x1e0); destruct `FUN_0028bcd8` 213411 (+0x200); dispenser `FUN_0028dfa0` 215118 (+0x1f0); grenade `FUN_0028f308` 215997 (+0x1ec) | cannon `FUN_00330810` 324833 (+0x240); destruct `FUN_00333110` 326434 (+0x260); dispenser `FUN_00336a50` 328987 (+0x254); grenade `FUN_00338190` 329976 (+0x254) |
| NextCharge 0x8316f444 | weapon base `FUN_002864f0` 209831; ordnance base `FUN_002738a8` 195850 | weapon base `FUN_00328fb0` 320507; ordnance base `FUN_00310620` 302473 |
| ChargeRateLight 0xf5701d6d | `FUN_002864f0` 210482 (+0xb0) | `FUN_00328fb0` 320883 (+0xe4) |
| ChargeRateHeavy 0x60ddbf8e | `FUN_002864f0` 210028 (+0xb4) | `FUN_00328fb0` 320167 (+0xe8) |
| MaxChargeStrengthHeavy 0xa61d280f | `FUN_002864f0` 210267 (+0xb8) | `FUN_00328fb0` 320630 (+0xec) |
| MaxChargeStrengthLight 0xe6c12d74 | `FUN_002864f0` 210447 (+0xbc) | `FUN_00328fb0` 320711 (+0xf0) |
| ChargeDelayLight 0x516c9f08 | `FUN_002864f0` 210174 (+0xc0) | `FUN_00328fb0` 320094 (+0xf4) |
| ChargeDelayHeavy 0xf303ac7b | `FUN_002864f0` 210521 (+0xc4) | `FUN_00328fb0` 320967 (+0xf8) |
| OrdnanceName 0xf8899c5e (and `Ordnance` 0x399bf05d, same branch) | weapon base `FUN_002864f0` 210544 -> 0x00286cd4 (+0x5c, range cache +0xd0); `FUN_00274ca0` 196533; dispenser `FUN_0028dfa0` 215167; launcher `FUN_00290758` 217017 | weapon base `FUN_00328fb0` 320904 (+0x88, cache +0x100); `FUN_00311620` 302957; dispenser `FUN_00336a50` 329050; launcher `FUN_0033acc8` 332070 |

- So every key is a **weapon-class** key, not a soldier key: the lines the brief flagged (B2 320094 / 320167, the
  MaxPressedTime lines) all sit in weapon setters. Class names come from the vtable order. Each setter's class
  vtable sits right after its class-name strings: B1 cannon 0x53b218 after "WeaponCannon" 0x53b1b8, destruct
  0x53b5b0 after "WeaponDestruct" 0x53b560, dispenser 0x53bb60 after 0x53bb08, grenade 0x53bf28 after 0x53bed8,
  launcher 0x53c2b8 after 0x53c268. B2: cannon 0x628330 / "WeaponCannon" 0x6282d0, destruct 0x628670 / 0x628620,
  dispenser 0x628c10 / 0x628ba8, grenade 0x628f78 / 0x628f28, launcher 0x6294c0 / 0x629470 (`refs.py` and the
  string scan). [V]
- The launchers inherit MaxPressedTime from the cannon setter: B1 `FUN_00290758` calls `FUN_00289b48`
  (216998 / 217019), B2 `FUN_0033acc8` calls `FUN_00330810` (332051 / 332072). [V] The detonator (BF2 detpack
  MaxPressedTime 0.5) goes through the dispenser setter: B1 `FUN_0028c3d8` -> `FUN_0028dfa0` (213737), B2
  `FUN_00334180` -> `FUN_00336a50` (327126). [D: the vtable address order puts these setters in the detonator
  class; not opened further]

## BF1

### 1. Parsing: how a stage list is built [V]
- Weapon setter `FUN_002864f0`, NextCharge (209831-209838):
  `child = this->vtbl[+0xc](this, this->name@+0x14); this+0x48 = child; child+0x44 = float(value)`.
  vtbl+0xc = `FUN_002885b8` (vtable 0x53b010, slot word 0x53b01c): alloc 0x1e0 and `FUN_00288538` -> copy ctor
  `FUN_00285d70`. The copy ctor copies the parent's fields (Ordnance +0x5c, the rumble keys +0xb0..+0xbc, …) and sets
  `+0x44 = parent+0x44`, `+0x48 = 0` (209571-209573). (Not copied: +0xc0 / +0xc4, ChargeDelay*. The B1 bowcaster
  and arc caster author both before their first NextCharge. [V ctor; D for effect] See Open questions.)
- Weapon class loader `FUN_00286258` (209720-209790): after each PROP chunk it calls the class's SetProperty
  (vtbl+0x20) on the *current* class, then `if (cur+0x48 != 0) cur = cur+0x48` (209765-209768). So every key
  after a NextCharge line goes to the new stage. At the end it walks the chain calling vtbl+0x24 (finalise) and
  sets the LAST stage's `+0x48 = itself` (209777-209787): the chain ends on a self-loop.
- Ordnance: the same mechanism. Setter `FUN_002738a8` NextCharge (195850-195857): `ord+0x2c = clone`,
  `clone+0x28 = value`. Ordnance loader `FUN_002736d0` (195732-195733): after every PROP, `cur = cur+0x2c`.
  Ordnance class ctors set `+0x2c = self` and `+0x28 = 0` (root, 195577-195579), or copy +0x28 from the parent
  (copy ctor `FUN_00273420`, 195631-195632).
- [D] A derived class does not inherit its base's stages. BASE resolves by name to the FIRST registered class with
  that hash (the list appends at the tail in `FUN_00285d70` 209549-209555; the lookup walks from the head,
  209727-209733), and that is the root. The TYPE clone sets +0x48 = 0. No disc class depends on this: every
  staged class authors its own NextCharge lines (survey).

### 2. Charging and stage choice [V]
- Instance: weapon+0x60 = root class, weapon+0x64 = current class.
- Charge start `FUN_00284f48` (instance vtbl 0x53ae38+0x80, word 0x53aeb8), called by every weapon kind's
  charge-enter wrapper (jal from 0x289068 cannon, 0x28b7b8, 0x28d7a0, 0x28ed48 grenade, 0x290328 launcher):
  `+0x64 = +0x60` (208888); starts the charge sound class+0x11c (`ChargeSound` 0x4ee58508, setter 210201);
  rumble reset: heavy strength +0xa0 = 0, heavy rate mult +0xb4 = 1, light +0xa4 = 0, light mult +0xb0 = 1,
  hold timer +0xb8 = class+0xc8, heavy delay +0xa8 = ChargeDelayHeavy (+0xc4), light delay +0xac =
  ChargeDelayLight (+0xc0). A negative ChargeRate* starts that motor at its max (208920-208934).
- Cannon charge-enter `FUN_00289050` (211479-211486): accum +0xd0 = 0, limit +0xd4 = MaxPressedTime (class+0x1e0).
  Cannon charge tick `FUN_00289080` (211492-211501): `accum = min(accum + dt, limit)`, then `FUN_00285108`.
- **Stage step `FUN_00285108`** (208944-208975): `f = accum / limit`; `while cur->next->NextCharge(+0x44) < f
  and cur->next != cur: cur = cur->next` (writes weapon+0x64). It can cross several stages in one tick. Callers:
  cannon 0x2890a8, 0x28b880, 0x28d7e0, grenade 0x28ed88, launcher 0x2904d8; 8 vtable words (`refs.py b1 285108`).
  So every weapon kind can have stages.
- Return to idle `FUN_00284a58` (208673): `+0x64 = +0x60`.

### 3. What the stage changes at release [V]
- Trigger init `FUN_00288c58` (211333-211344): `charge +0x140 = accum / cur.MaxPressedTime` (1.0 if <= 0).
- Shot `FUN_002892a8`: the per-shot offset array (class+0x23c / count +0x240), ShotsPerSalvo (+0x230, fire
  cycle `FUN_00288d10`), heat, spread all come from `*(weapon+100)` = the current stage (211728-211741). The
  packet `.x` at +0x20 = charge (211784). Spawn through the cannon's vtbl+0xc4 = 0x289278 (disassembled):
  `lw v0,100(a0); lw a0,92(v0); jalr ordClass->vtbl[+0x10]`. The ordnance is the **current stage's** +0x5c.
- Grenade throw `FUN_0028ef70` (215914-215916): ordnance = `*(weapon+100)+0x5c`, packet charge = weapon+0x144
  (strength = charge time / MaxPressedTime, 1.0 when that is 0; re-grenades §3.2).

### 4. What the charge changes inside the ordnance [V]
- Instance base ctor `FUN_002728a8` (195082-195112): starting from the class it was spawned with, step `ord+0x30`
  along +0x2c while `charge > next.+0x28`; then `ord+0x34 = (charge - t_i) / (t_next - t_i)`, or 0 when
  `t_next <= t_i` (last knot, or no chain: +0x2c = self). LifeSpan: `ord+0x3c = lerp(cls+0x30, next+0x30, s)`
  (195130-195133).
- Bolt-family spawn `FUN_002762a0` (197532-197543): `velocity = dir x lerp(Velocity +0xac, next.Velocity, s)`.
  Called by 7 instance ctors (bullet, laser, missile `FUN_0027d3f0`, shell `FUN_0027e398`, and others).
- Shell gravity `FUN_0027e398` (203470-203473): lerp(+0xdc). Missile update `FUN_0027d6b8` (202940-203108):
  TurnRate / WaverTurn / WaverRate lerp.
- Sticky inherits the same spawn evaluation: `FUN_0027eb18` calls shell at B1:203821;
  shell calls bullet at 203465; bullet calls base at 197521 and blends speed at
  197533-197546. The base stores blended life at 195130-195133. BF2 follows the
  same chain: `FUN_003207e8` at B2:313521, `FUN_003201f0` at 313265,
  `FUN_003130d8` at 304011 and 304023-304036, `FUN_0030eef8` at
  301397-301400. A sticky's later fuse arming does not reload root life. [V]
- Damage descriptor `FUN_00272f88` (195397-195424): MaxDamage (+0x4c), 7 scales (+0x50..+0x68) and ShieldScale
  (+0x6c) lerp toward next by `s`, but only if `s > 0`.
- Emitter (arc caster) create `FUN_00278208` (199074-199075): `D = lerp(cls+0x94, cls+0x98, packet charge)`. This
  is the raw charge, not `s` (there is no instance). Per re-ordnance-kinds §8 these are Damage -> MaxDamage, and
  the hit chain then decides the target count.
- AI lead getters `FUN_00283de8` / `FUN_00283f00` (weapon vtbl+0x2c / +0x30): while state 4 (charging) they
  return the same knot lerp of the ordnance's speed / gravity at the live fraction.

### 5. Light / Heavy = rumble [V chain, D semantics]
- In `FUN_00285108` (208976-209014), gated `owner+0xe8 >= 0` (a player index):
  `heavyDelay -= dt; lightDelay -= dt;` when heavyDelay <= 0: `heavy += ChargeRateHeavy x dt x +0xb4`, capped at
  MaxChargeStrengthHeavy. When it reaches the cap, the hold timer +0xb8 (class+0xc8) runs down, and at 0 both
  rate multipliers become -1.0 (`SUB_bf800000`), so both motors ramp down. Light works the same way with its own
  delay, rate and cap. Each tick: `FUN_00349c88(0.1, 0.1, heavy, light, 0, 0, 0x642e40, owner+0xe8)`.
- `FUN_00349c88` -> *(decompiler detail omitted)*: block[4] = light,
  block[5] = heavy x 0.7, 0.1 s pulse (334953). Pad update `FUN_0034aed0` feeds block[4] to `FUN_004b4a10`
  (`pad+0x21c = value > 0`, an on/off motor) and block[5] to `FUN_004b49d0` (`pad+0x21d = clamp01(value) x 255`).
  The DualShock has a small on/off motor and a large 0-255 one, so Light = small, Heavy = large. [D]
- Key 0x0ee66c41 (name not cracked) writes the hold time +0xc8 and also TriggerSingle +0xcc (209915-209917 ->
  0x00287124 210358-210360). Not authored on either disc (STTAP card).
- `ChargeSoundPitch` 0x12586dae (+0x124, FNV-cracked): while charging, the charge sound's +0x40 param =
  `((1 - f) x ChargeSoundPitch + f) x base`, clamped to [0.2, 0.4] (209017-209026). [V code; H meaning]

## BF2

Every BF1 step has a BF2 twin. I opened each one:

| step | B2 function, lines | layout change |
|---|---|---|
| weapon NextCharge | `FUN_00328fb0` 320507-320514 | next +0x70, threshold +0x6c (BF1 +0x48 / +0x44) |
| loader follows the stage | `FUN_00328d08` 319871-319874; tail self-loop 319835 / 319845 | — |
| ordnance NextCharge / loader | `FUN_00310620` 302473-302481; `FUN_00310468` 302304-302305 | +0x2c / +0x28, the same |
| charge start | `FUN_00326f88` 318705 (`+0x64 = +0x60`), rumble init 318740-318758 | ChargeSound +0x148, ChargeDelayHeavy +0xf8 -> +0xa0, ChargeDelayLight +0xf4 -> +0xa4, hold class+0xfc -> +0xb0 |
| cannon charge tick | `FUN_0032ed98` 323675-323683: clamp accum +0xbc at +0xc0 | accum +0xbc, limit +0xc0 = class+0x240 |
| stage step + rumble | `FUN_00327168` 318777-318841 | identical algorithm; rumble `FUN_00432d90` -> `FUN_00433b28` (the same block layout, heavy x 0.7) |
| idle reset | `FUN_003269f8` 318426 | — |
| trigger init | `FUN_0032e758` 323450-323455: charge +0x120 = accum / class+0x240 | — |
| cannon spawn | `FUN_0032f208` 324126 (packet charge), create slot 0x32f1d8 (disassembled: `lw 100(a0); lw 136(v0)`; ordClass vtbl+0x10) | ordnance +0x88 |
| grenade throw | `FUN_00337b80` 329884-329886 (ordnance = cur+0x88), charge = weapon+0x124 (329820) | — |
| ordnance knot pick | `FUN_0030eef8` 301352-301384; LifeSpan lerp 301398-301401 | — |
| bolt velocity | `FUN_003130d8` 304023-304024 (Velocity +0xe4) | — |
| damage | `FUN_00310178` 302180-302191: damage +0x70, **8** scales +0x74..+0x90, shield +0x94 lerp by `s`; the age falloff (+0x34..+0x3c) and team bonus (+0x40) read the knot's own values, not lerped | — |

The stage-step callers in B2 cover every kind: `FUN_0032bda0`, cannon `FUN_0032ed98`, `FUN_003327f8`,
`FUN_00335718`, grenade `FUN_00337948`, `FUN_0033a950` (`re_check.py callers b2 FUN_00327168`). [V]

## Differences

- Code: the same design. Offsets move: the weapon chain is BF1 +0x48 / +0x44, BF2 +0x70 / +0x6c; Ordnance +0x5c ->
  +0x88; rumble keys +0xb0..+0xc4 -> +0xe4..+0xf8. BF2's damage lerp has 8 scales (BF1 7) and BF2 adds the
  age falloff after the lerp. [V]
- BF2 only (`FUN_00327168` 318843-318880): when accum > limit and flag bit 1 of weapon+0x108 is clear, it plays a
  "charged" sound once (class+0x154) and may stop the charge loop (class flag bit 61). The cannon tick clamps
  accum to the limit, so for the cannon this never fires [D]. The kinds whose tick passes (argument 1, argument 2)
  (`FUN_003327f8`, `FUN_0033a950`) were not opened.
- BF2 trigger init adds a wind-up (+0x2ac) and uniform(min,max) ShotDelay before the charge line (323439-323457).
  It does not change the charge.
- Data:
  - The bowcaster is inverted: BF1 taps one bolt and charges into a 3 / 5 / 7-bolt fan; BF2 taps the 7-bolt fan
    (75 dmg, 150 m/s) and charges into one 300-dmg, 600 m/s bolt.
  - The mortar's ordnance names are swapped (BF1 tap `_ord1` shell / hold `_ord` sticky; BF2 tap `_ord` shell /
    hold `_ord1` sticky), but it behaves the same.
  - The BF1 arc caster has 5 heat stages; the BF2 arc caster has no stages (charge acts only through the emitter).
  - BF2 adds the dish-turret knot chain.

## Survey table

From engine/assets_baked/classes (props in authored order, `base` chained; script
`scripts/tools/charge_survey.py table`). Listed: every `wpnc` with MaxPressedTime > 0 or a NextCharge, and every
`ordc` with a NextCharge. Rumble = Light rate / max / delay; Heavy rate / max / delay. "Stages": thr = the stage's
NextCharge (stage 0 has none), then the ordnance (its engine base, Velocity, Damage) and the shots.

| game | weapon (engine kind) | MaxPressedTime | stages | rumble L / H |
|---|---|---|---|---|
| BF1 | all_weap_inf_bowcaster (cannon) | 2.0 | 0: `_ord` bolt 100 m/s 60 dmg x1; 0.15: `_ord_high` bolt 100 m/s 75 x1; 0.35: `_high` x3 (yaw 0, ±0.8); 0.65: x5 (±0.8, ±1.6); 0.95: x7 (±0.8, ±1.6, ±2.4) | 1/1/0.15; 1/0.5/2.0 |
| BF1 | imp_ / cis_weap_inf_mortar_launcher (grenade) | 0.75 | 0: `_ord1` shell 40 m/s, Gravity 1.75, 150 dmg; 0.9: `_ord` sticky 40 m/s, LifeSpan 0.5 | 1/1/0.1; 1/0.5/0.75 |
| BF1 | rep_weap_inf_arccaster (cannon) | 2.25 | the same `_ord` (emitterordnance, MaxDamage 250) in all 5; HeatPerShot 0x67cffd1a 0.42 / 0.6 (0.25) / 0.65 (0.5) / 0.8 (0.75) / 1.0 (0.99) | 1/1/0; 1/0.5/2.2 |
| BF1 | imp_ / cis_ / rep_ / all_weap_inf_remotedroid_destruct (destruct) | 3.0 | none (no ordnance) | — |
| BF2 | all_weap_inf_bowcaster <- com_weap_inf_bowcaster (cannon) | 1.25 | 0: `_ord` bolt 150 m/s 75 dmg x7 (yaw 0, ±0.7, ±1.4, ±2.1); 0.95: `_ord_high` bolt 600 m/s 300 dmg x1 (count 1, no pair re-authored) | 1/1/0.15; 1/0.5/2.0 |
| BF2 | all_ / cis_ / imp_weap_inf_mortar_launcher <- com_weap_inf_mortar_launcher (grenade) | 0.75 | 0: `_ord` shell 40 m/s, G 1.75, Damage 400, LifeSpan 8; 0.9: `_ord1` sticky 40 m/s, Damage 100, LifeSpan 0.1 | 1/1/0.1; 1/0.5/0.75 |
| BF2 | imp_weap_inf_arccaster <- com_weap_inf_arccaster (cannon) | 2.5 | none; `_ord` emitterordnance Damage 300 / MaxDamage 325 | 1/1/0; 1/0.5/2.2 |
| BF2 | tur_weap_hoth_dishturret <- com_weap_veh_recoilless (cannon) | 2.0 | none at the weapon; the ORDNANCE has knots (next row) | 1/1/0; 1/0.75/2.0 |
| BF2 | com_ / all_ / cis_ / imp_ / rep_weap_inf_remotedroid_destruct (destruct) | 3.0 | none | — |
| BF2 | com_ / all_ / cis_ / imp_ / rep_weap_inf_detpack (detonator) | 0.5 | none (`_ord` = mine) | — |

Ordnance knot chains (`ordc` with NextCharge):

| game | ordnance (base) | knots (threshold: values) |
|---|---|---|
| BF2 | tur_weap_hoth_dishturret_ord <- com_weap_veh_recoilless_ord (bolt, 600 m/s) | 0: Damage 1000, LaserLength 3, LaserWidth 0.3; 0.5: 1250, 4, 0.4; 0.75: 1500, 5, 0.5; 2.0: 3000, 6, 0.6. Charge <= 1, so the last segment reaches s = 0.2 -> Damage 1800 at most [D] |
| BF1 | gun_weap_inf_haywiredetonator_ord (grenade) | 0: Velocity 1.5; 1.0: Velocity 25. Its weapon has no MaxPressedTime, so strength = 1.0 -> s = 1 -> always 25 m/s [D]; re-grenades §9 table (line 524) lists 25 |
| BF1 | ewk_weap_inf_rock_ord (shell) | 0: Velocity 5; 1.0: Velocity 20; LifeSpan 3 throughout. No press limit, so strength 1 selects 20 m/s [D] |
| BF1 | ewk_weap_inf_spear_ord (sticky) | 0: Velocity 5; 1.0: Velocity 25; LifeSpan 2 throughout. No press limit, so strength 1 selects 25 m/s [D] |

Classes that author rumble keys with MaxPressedTime 0 (e.g. BF1 crack grenade) have no charge. Their rumble
code never runs [D: the charge state needs MaxPressedTime > 0]. The full key dump is
the output of `scripts/tools/charge_survey.py`.

Carried by: BF1 bowcaster = Wookiee classes (bf1_all.ron 5940 / 6060); arc caster = rep (2506); mortars = IMP /
CIS pilot classes (bf1_imp 3036 / 3520 / 3641, bf1_cis 3557 / 4421). BF2: mortars = imp / cis officer
(bf2_imp 7844, bf2_cis 8800), all Wookiee (bf2_all 10553); bowcaster = Wookiee + Chewbacca; arc caster = dark
trooper (bf2_imp 5559); dish turret = Hoth turret.

## The port (issue #29)

What the port does with each weapon feature #29 lists, on both games (main after the issue's three slices):

| feature | PS2 rule (this note, re-ordnance-kinds.md) | port | where |
|---|---|---|---|
| Charge accumulator | a fresh press starts it at 0, limit MaxPressedTime; charge = held / MaxPressedTime | built (before: the limit stayed the shot delay, so a held charge stopped at ShotDelay / MaxPressedTime) | `core::weapon::WeaponState::update` |
| Weapon charge stages | NextCharge clones the class; the shot is the stage reached | built: BF1 bowcaster 1 -> 3 -> 5 -> 7 bolts, BF2 bowcaster fan -> one heavy bolt, mortars shell -> sticky, BF1 arc caster heat steps | `game::roster::weapon_from_db`, `WeaponClass::stages` |
| Per-stage shot offsets | each stage's count key and pairs | built (before: the first table only) | `weapon_stage` |
| Ordnance charge points | Velocity, LifeSpan, damage + scales blend by the charge's place in its segment | built (BF2 Hoth dish turret 1000 -> 1800; BF1 haywire always 25 m/s) | `OrdnanceClass::charged` |
| Emitter damage | lerp(Damage, MaxDamage, charge) | built for the first target's damage (BF2 arc caster 300 -> 325) | `OrdnanceClass::min_damage` |
| Emitter chain | targets and jumps by the charge (re-ordnance-kinds §8) | **not built**: the arc caster still fires a bolt; card UORDE | - |
| Shell | Gravity x 9.8, meets the world on its arc, ExplosionExpire in the air | built (before: a straight bolt) | `Bolt::shaped`, `Sim::fly_shells` |
| Heat / cooldown | HeatPerShot, HeatRecoverRate, overheat at 1, clear under HeatThreshold | built earlier (card heat), per stage now | `WeaponState::update` |
| Clips and ammo | RoundsPerClip, WeaponAmmo clips, ReloadTime | built earlier (card AMMO); every class's clips checked against the discs by `roster::every_class_kit_is_the_discs_or_explained` | `game::roster` |
| Rumble (Light / Heavy) | the two controller motors while charging | **not built** (no rumble in the port) | - |
| Uncharged throws | thermals and Ewok rock/spear have no press limit: strength is 1.0, including for ordnance knots; mortars have the stages above | sticky launch uses the shot's evaluated speed and life, as the shell path already does | `core::weapon`, `Sim::launch` |

Before the issue the port read a class key's FIRST value in the weapon's own class, so a charge weapon's
every shot was its stage 0 (the first OrdnanceName and ShotsPerSalvo) - and since the charge never got past the
shot delay, no stage was ever reached.

## Open questions

1. **re-ordnance-kinds.md conflicts with the data.** Its table row says the BF2 mortar is "one stage, shell", but
   the bake has `NextCharge 0.9` -> `_ord1` (sticky) in all three BF2 mortar classes, and the code path is the
   same as BF1's. Check with a PS2 hold-fire sample on a BF2 officer before building.
2. The copy ctor `FUN_00285d70` does not copy +0xc0 / +0xc4 (ChargeDelayLight / Heavy) into a stage. What a stage
   holds there (the plain ctor default?) matters only for rumble, so it is low priority. BF2's copy ctor was not
   checked for the same gap.
3. The BF2 bowcaster's stage 1 sets count 0xa4dfafc0 = 1 and authors no pair after it. It is unknown whether the
   offset array is reallocated (zeroed) or keeps the cloned 7-pair array; this sets the charged bolt's direction
   offset (probably 0, 0). [H]
4. Destruct (MaxPressedTime 3.0) and detonator (0.5) use MaxPressedTime for a hold-to-trigger. Their own logic
   was not opened.
5. How a bot holds a charge weapon (press length / release rule) was not looked at. It decides which stage AI
   mortars and bowcasters fire.
   A weapon with no MaxPressedTime fires every shot at charge 1.0 (the trigger init), so the port's AI reads
   such a weapon's speed and reach at charge 1.0 (`WeaponClass::shot_ordnance`, 2026-11-04, issue #82). Before that, the
   Gungans' haywire read the uncharged 1.5 m/s, a 1.35 m reach, and its throwers never fired. A charging weapon
   is still read at charge 0.
6. Key 0x0ee66c41 (rumble hold + TriggerSingle) and BF2 class+0x154 ("charged" sound key) are not named. The
   ChargeSoundPitch clamp [0.2, 0.4] reading is a hypothesis.
7. Not verified on a PS2 here: the stage thresholds (e.g. a mortar tap vs a 0.7 s hold) — a 2-minute RAM read of
   weapon+0x64 / +0xd0 would settle it.
