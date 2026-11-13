# RE: BF1 passive aim-assist magnetism (SWBFRONT.ELF)

> **§3 rewritten 2026-09-28 (card AIMF): OPUS RE - [O] UNAUDITED.** The soldier's magnet target is chosen by the
> PlayerController update FUN_0025e098: a cone around the aim ray, a team/kind filter, an LOS ray and a sticky current
> target. It is not a ray hit. §3 is the transcription, checked against 23 BF1 save states (§3.9). The old §3 (a ray,
> no team filter) is the MountedTurret's and moved to §3.10. §1, §2, §4, §8 and §9 are corrected where they relied on it.
>
> **§10 added 2026-10-02 (card AIMA): OPUS RE - [O] UNAUDITED.** The aim ray itself: the camera looks `TiltValue` below
> the aim (BF1 5°, BF2 10°), the ray leaves the camera's position, the reticle is drawn where it goes on screen, and
> the shot flies from the soldier's fire point at what the ray hits. Both games, with PCSX2 measurements.

Status: core algorithm fully recovered from decomp (2026-08-30). All addresses
are EE virtual addresses in `analysis/SWBFRONT.ELF-decomp.c`. Constants below
are exact as they appear in the decompiled float literals. Items marked
**[verified]** were read directly from function bodies; **[hypothesized]** are
inferences that a PCSX2 trace should confirm before the Rust port hardens them.

## 1. End-to-end summary

BF1 has no explicit lock-on for blasters. Its "aim assist" is three passive
effects applied to the *controller stick values* of the local human player,
every controller-update tick, whenever the player's current weapon has a
**magnet target**:

1. **Sticky slowdown** — both aim axes (yaw and pitch sticks) are multiplied
   by **0.5** whenever a valid magnet target exists. Unconditional (applies
   even with the Auto Aim option OFF and in MP).
2. **Strafe/velocity tracking (yaw only)** — a feed-forward term is added to
   the yaw stick that matches the lateral angular velocity of the target
   relative to the player, scaled by how hard the player is pushing the move
   stick. This is what keeps the reticle glued to a target while strafing
   past it. Also unconditional.
3. **Fire magnetism (yaw only)** — while the fire button is held (and for a
   1-second decay after release), the yaw stick is pulled toward the
   *ballistically-led* aim point of the target at up to 75% of the remaining
   angle per clamp, with a linear distance falloff out to the weapon's
   `LockOnRange`. This is the only part gated by the Auto Aim option (SP) or
   the host's `aimassist` session setting (MP).

Pitch is never pulled — only slowed. All corrections are expressed in stick
units (post-clamp [-1,1] axis values), *before* the game turns stick values
into rotation, so the assist can never rotate faster than the soldier's max
turn rate.

The magnet target is the best-scored live object in a narrow cone around the
aim ray, per weapon channel (§3). It must pass the weapon's Target* team/kind
filter (by default: enemies only), have a clear line of sight, and lie within
`LockOnRange` + its bounding radius. Once held, it stays while it remains in a
cone widened by π/4 while fire is held. A friendly or a neutral is never the
magnet, though it can be the reticle's pick (red/green/blue, §3.7).
`LockOnRange <= 0` disables the magnet for that weapon. The class default is 0,
but every BF1 infantry blaster sets 64 (§3.8). *(Corrected 2026-09-28, card AIMF:
this paragraph used to say "the nearest object hit by a ray", which is the
MountedTurret's rule, §3.10.)*

## 2. Function map

| address | role | confidence |
|---|---|---|
| FUN_0025fea8 | **Aim-assist core**: slowdown + tracking + fire magnetism applied to stick values | verified (read in full) |
| FUN_0025e098 | **PlayerController** update (vtable 0x538500 slot, typeinfo FUN_00260798 = `16PlayerController`): decodes 4 analog axes + button counters, calls FUN_0025fea8, writes axes to entity +0x60..0x6c, then **acquires the magnet target per weapon channel (§3)** | verified |
| FUN_00241b98 | The cone score `t + 16·d²` (§3.4); also the vehicle-entry cone (B1:119544) | verified (instructions) |
| FUN_00221788 | Relation of B as seen by A: −1 enemy / 0 neutral / 1 friend, from the team table (§3.5) | verified |
| FUN_00223850 | **MountedTurret** tick (not the soldier's): a LockOnRange ray, no team filter (§3.10) | verified (targeting part) |
| FUN_0023d708 | Get current weapon object of a controllable (`vtbl294[0x0e]`/`[0x0f]` on `entity+0x180` index) | verified |
| FUN_0023d760 | Returns current weapon index (`entity+0x180`) | verified |
| FUN_0023c2a8 | Raw setter `weapon+0x128 = target` | verified |
| FUN_0023d1f8 | `SetTarget(entity, tgt)` with **TargetEnemy/Neutral/Friendly** (class+0xf4) and **TargetPerson/…/Building** (class+0xf8) bit filters; used by AI (callers in 0x24xxxx AI region) | verified |
| FUN_00161d68 → FUN_00162c60 | Ray query wrapper: cast `range` along `dir`, default collision mask **0x5e**, returns hit fraction + hit object | verified |
| FUN_00162b60 | Ray-query per-object callback (exclusion list + nearest fraction) | verified |
| FUN_0026d3f0 | **Ballistic lead solver**: given ordnance speed/gravity (from `weapon+0x6c` fire-point obj `+0x148` ordnance class, vtbl+0x2c speed / +0x30 gravity), target aim point and target velocity, returns intercept point | verified |
| FUN_001ac6f8 | 4x16B matrix assembler (rows: lateral, up, forward, position) | verified |
| FUN_001645a0 / FUN_001645a8 | Collision-prim radius (`prim+0xc`) / position accessor | verified |
| FUN_00434110 | fabsf (`& 0x7fffffff`) | verified |
| FUN_002864f0 | **WeaponClass ODF parser** (hash-dispatch; see table §4) | verified |
| FUN_00225f08 | Soldier-class ODF parser (`MaxTurnSpeed`/`TurnRate` → class+0x113c, `PitchLimits` → +0x112c/+0x1130 deg→rad, `PitchTurnFactor` → +0x1110) | verified |
| FUN_0034d638 / FUN_0034d798 | Current-profile getter; profiles at `0x00643718 + idx*0x2e4`, idx globals `DAT_00519260`/`DAT_005187fc` | verified |
| ScriptCB_IsAutoAimOn @00179c30 / ScriptCB_SetAutoAimOn @00179c88 | Lua shell get/set of profile flag `profile+0x34` | verified |
| ScriptCB_SetNetGameDefaults @001866c0 | 4th numeric arg → `DAT_00518bdc` (MP aim-assist enable); surfaced to the session browser as the `"aimassist"` key in ScriptCB_GetSessionList @00188b58 | verified |
| FUN_002d4b50 | HUD reticle draw. It colours the reticle red / green / blue by the acquisition's relation K+0x40+4·ch (§3.7) and uses `LockOffAngle` (class+0xdc) to size the lock ellipse | verified (skimmed; colour branch read 2026-09-28) |
| FUN_0028f7c0 | Weapon salvo/recoil (class+0x64/+0x68 recoil into weapon +0x98/+0x9c) — unrelated, listed to avoid re-confusion | verified |
| `aimtension` console cmd @001b6b68/001b7390 | Camera AimTension debug command (camera smoothing, **not** aim assist) | verified |

## 3. Target acquisition — the soldier's magnet (FUN_0025e098) — OPUS RE (loop lane 2, 2026-09-28, card AIMF) - [O] UNAUDITED

Conventions. B1 means lines of `analysis/SWBFRONT.ELF-decomp.c`, B2 lines of `SWBFRNT2.ELF-decomp.c`. **K** is the
Controllable the player drives (*(decompiler detail omitted)*). For a soldier, K = soldier object O + 0x2a0: the soldier's Controllable
vtable 0x532be0 sits at K+0x294 = O+0x534, and the vtable's first word −0x2a0 is that offset [V-RAM]. O is `units.py`'s
health component C − 8. **W** is the channel's weapon; **C** is W's class (W+0x64). Tags: [V] read; [V-instr] read from
the ELF instructions; [V-RAM] measured in PCSX2 save states (§3.9); [D] derived; [H] open. [O] marks the load-bearing
claims, those the checks and the verifier covered (§3.9).

### 3.1 Where and when [V][O]
- FUN_0025e098 is the **PlayerController** update. Its one data reference is the slot 0x53851c of the vtable at
  0x538500, whose +4 typeinfo FUN_00260798 registers `16PlayerController` (0x538558).
- Each frame runs in this order:
  1. The assist core FUN_0025fea8 (§5) runs first (B1:179737), on the weapon of the CURRENT channel K+0x180
     (FUN_0023d708) and **last frame's** magnet W+0x128.
  2. The button counters update. Channel 0's fire counter is the s8 at K+0x48, from pad bit 0 (B1:179743-179790);
     channel 1's is K+0x4a, from bit 1. A counter goes to 2 on the press and counts up each held frame to 100. On
     release it goes to −1, then counts down to −100.
  3. The acquisition below runs for channel 0, then channel 1 (B1:180428-180916; the exit is *(decompiler detail omitted)*, then
     FUN_00260548). Each channel uses its own weapon: K vtbl+0x38(ch) gives a weapon index (soldier: K+0x930+4·ch), and
     vtbl+0x3c(i) gives the weapon (K+0x910+4·i) (B1:180429, 180431; the soldier's slots 0x210ce0 / 0x210cf0 [V-instr]).
- So on the PS2 the assist of frame N pulls toward the target acquired on frame N−1 [D].

### 3.2 Inputs [V][O]
- **pos** = K+0x00 and **dir** = K+0x110 (B1:180448-180453). **self** = K vtbl+0x18() (B1:180463); for a soldier this
  is O (0x210c60: `addiu v0, a0, -672`).
- [V-RAM] K+0 = feet + (0, 2.104, 0) ± 0.02 m on a standing player (6 states). K+0x110 is the crosshair direction: its y
  is −0.070 where the camera forward's is −0.157, i.e. ~5° above the camera axis, as ps2live measured. The crouch and
  prone origins were not sampled [H]. *(2026-10-02, §10.2: K+0 is a point on the ray from the camera's position along
  K+0x110 — 2.104 m is its height at the spawn pitch, not a constant — and the 5° is the class's TiltValue.)*
- Weapon class fields (B1:180457-180461):

  | offset | field | notes |
  |---|---|---|
  | C+0xd0 | ExtremeRange | fnv 0x8474ea2c; the name found by brute force: fnv("ExtremeRange") = 0x8474ea2c |
  | C+0xd4 | LockOnRange | |
  | C+0xd8 | LockOnAngle | radians: the ODF's degrees × 0.017453294, or AutoAimSize × 0.072 |
  | C+0xdc | LockOffAngle | |
  | C+0xf4 | team bits | TargetEnemy 1 / Neutral 2 / Friendly 4 |
  | C+0xf8 | kind bits | TargetPerson 1 / Animal 2 / Droid 4 / Vehicle 8 / Building 0x10 |

- WeaponClass ctor FUN_00285a98 defaults (B1:209490, 209497, 209504-209507):
  - +0xf8 = 0xffffff9f: kinds 0-4 on, kinds 5 and 6 off.
  - +0xf4 = 1: enemies only.
  - +0xd0..+0xdc = 0.
- The setter FUN_002864f0:
  - ExtremeRange and LockOnRange each end in `if (+0xd0 < +0xd4) +0xd0 = +0xd4` (B1:210249-210258; instructions
    0x287138-0x287170).
  - **OrdnanceName** (0xf8899c5e, B1:210544 → 0x00286cd4, B1:209842-209855) finds the ordnance class and sets
    `+0xd0 = ordnanceClass->vtbl(+0x1c)[0x2c]()`, with no clamp.
    - The rifle's ordnance, the `bolt` label, is an OrdnanceLaserClass: vtable 0x53a4f0 [V-RAM].
    - Its slot +0x2c (0x53a51c) is FUN_00277d30, inherited from OrdnanceBulletClass (vtable 0x539b90). It returns
      Velocity (+0xac) × LifeSpan (+0x30) [V-instr: `lwc1 $f1,48(a0); lwc1 $f0,172(a0); mul.s`].
    - The base OrdnanceClass slot returns 0.0 (FUN_002743a0).
    - [V-RAM] C+0xd0 equals that product on three ordnance classes: 80 × 1.5 = 120, 50 × 3.0 = 150 and
      25 × 0.9 = 22.5.
    - *(The verifier corrected the class: an earlier draft named 0x539b90 as the bolt's own vtable.)*
  - The cannon setter FUN_00289b48 does not handle OrdnanceName: it falls through to FUN_002864f0 (B1:212206). The
    dispenser (FUN_0028dfa0) and launcher (FUN_00290758) setters handle it themselves.
  - Every BF1 blaster lists OrdnanceName **after** LockOnRange, so its ExtremeRange is the bolt's reach. The rifle's is
    80 m/s × 1.5 s = **120 m**. [V-RAM] C+0xd0 = 120.0 on the player's rifle in 22 of 23 states; other weapons read
    112.5, 150 and 320.

### 3.3 The algorithm, per channel (B1:180428-180916) [V][O]
```
target = W+0x128; if (target && target+0xb4 != W+0x12c) { W+0x128 = W+0x12c = 0; target = 0 }   // 180433-180442
hint = W+0x130; valid = (target != 0); best = FLT_MAX (0x5384fc = 0x7f7fffff)
if (target) {                                                   // the sticky current target, 180466-180641
    if (0 < C.LockOffAngle && W.vtbl(+0x14)[0x34](W)) { cone = LockOffAngle; held = 1 }       // 180467-180472
    else { held = 0; cone = LockOnAngle; if (s8 K+0x48+2ch > 0) cone += 0.7853982 }           // 180479-180482; 0x3f490fdb @0x25f4a8
    (ctr, r) = Bounds(target)                                                               // §3.6, 180485-180553
    s = ConeScore(r, cone, C.ExtremeRange, pos, dir, ctr)                                   // 180554
    if (s >= 0 && Eligible(target)) {                                                       // 180555-180565
        if (!held) hint = target.vtbl[0x58](target, pos, dir)                               // 180568
        aim = target.vtbl[0x5c](target, pos, dir, hint)                                     // 180571
        end = aim + r·normalize(pos − aim)                                                  // 180578-180593
        if (!Ray(pos → end, exclude {self, target}, mask 0x48)) {                           // 180600 FUN_00161de8
            valid = Passes(target) && |ctr − pos| <= LockOnRange + r                        // 180601-180622
            best = held ? 0.0 : s                                                           // 180624-180626
        } else { target = 0; hint = −1; valid = false; best = FLT_MAX }                     // 180628-180633
    } else { target = 0; hint = −1; valid = false; best = FLT_MAX }                         // 180637-180641
}
if (0 < best) {                                                                             // 180647
    centre = pos + dir·(ExtremeRange·0.5); inv = Inverse(self.vtbl[0x40]())                // 180648-180670 FUN_00446608
    list = BoxQuery(half-extent ExtremeRange, centre, max 300, exclude self, mask 0x1e)     // 180671 FUN_00161c00
    for cand in list {                                                                      // 180675-180879
        if (!Eligible(cand)) continue                                                       // 180679-180690
        pass = Passes(cand)                                                                 // 180691-180694
        if (valid && !pass) continue                                                        // 180696
        (ctr, r) = Bounds(cand)
        if (inv·ctr lies inside self's local model box) continue                            // 180766-180807
        inrange = pass && |ctr − pos| <= LockOnRange + r                                    // 180808-180829
        s = ConeScore(r, LockOnAngle, ExtremeRange, pos, dir, ctr)                          // 180831 (never widened)
        if (0 <= s && s < best) {                                                           // 180833
            h = cand.vtbl[0x58](...); aim = cand.vtbl[0x5c](..., h); end = aim + r·normalize(pos − aim)
            if (!Ray(pos → end, exclude {self, cand}, mask 0x48))                            // 180865
                { hint = h; best = s; target = cand; valid = inrange }                      // 180866-180871
        }
    }
}
if (valid) { W+0x128 = target; W+0x12c = target ? target+0xb4 : 0; W+0x130 = hint }         // 180882-180889
else { W+0x128 = W+0x12c = 0; W+0x130 = −1; if (target && health(target+8) <= 0) target = 0 }   // 180891-180896
K+0x30+8ch = target; K+0x34+8ch = target ? target+0xb4 : 0                                 // 180898-180904
K+0x40+4ch = target ? Relation(self, target) : 0                                            // 180906-180913
```
The helper predicates, with kind(o) = o+0xac & 7:
- **Eligible(o)** = ((o+0xac bit 3) || kind(o) ∈ {5, 6}) && o+0xe0 != 0 && o.vtbl[0x90](o, self) != 0
  (B1:180556-180565, 180679-180690).
- **Passes(o)** = (C+0xf4 >> (Relation(self, o) + 1)) & 1 && (C+0xf8 >> kind(o)) & 1 (B1:180602-180606,
  180691-180694; `srav` at 0x25f948 and 0x25f964 [V-instr]).
- **Ray**: FUN_00161de8 → FUN_00162e88. It returns non-zero when something blocks, and it tests the ordnance subset of
  bodies (re-collision-layers.md §4); mask 0x48 is building | terrain.
- **BoxQuery**: FUN_00161c00 → FUN_00162a98, an AXIS-ALIGNED BOX of half-side ExtremeRange (not a sphere). It collects
  each hit body's owner (`+0xc0` vtbl+0x34), skips self, and stops at 300.

What follows from the code [D]:
- **A friend or a neutral never becomes W+0x128 under the default +0xf4 = 1.** It can still be K+0x30, the reticle's
  pick.
  - [V-RAM] 4 states hold a friendly pick (rel 1) with W+0x128 = 0.
  - Caveat from §3.5: an object neutral by Team but enemy by PerceivedTeam would get rel −1. No soldier on the discs
    is like that.
- **The pick depends on list order.** Once a valid target is held, only filter-passing candidates may replace it (line
  180696). While none is held, every eligible candidate competes on score.
  - So a friend who scores better than an enemy wins if the query returns it first, and the enemy then cannot beat it:
    **no magnet**.
  - If the enemy comes first, the friend is skipped and the enemy is the magnet.
  - The list walks the collision manager's layer lists 2, 4, 8, 0x10 in that order (FUN_001627d0). The order inside a
    list was not traced [H].
- **The sticky target** keeps the plain score t + 16·d², but only the widened cone gates it. A newcomer needs a strictly
  lower score inside the unwidened cone and must pass the filter.
- **The LockOff path** (held = 1) sets best = 0, so no query runs at all: the target is kept while it stays in the
  LockOffAngle cone.
  - Only weapons with LockOffAngle > 0 take this path: BF1's launchers and the K-wing missile (10°) [V disc census].
  - A launcher's W.vtbl[0x34] is its dwell-lock flag W+0x168 (FUN_002909f0); the base's (cannons) is "holds a valid
    target" (FUN_00288168) [V-instr; weapon vtables from RAM].
- **Between LockOnRange + r and ExtremeRange + r** an object can be the reticle's pick but never the magnet. If such a
  filter-passing object scores better than the held target, it replaces it with valid = false and the magnet drops.

### 3.4 The cone score FUN_00241b98(r, slope, maxd, pos, dir, ctr, mode) [V-instr][O]
From the instructions at 0x241b98-0x241c84:
```
v = ctr − pos (xyz);  t = v·dir (x + y + z)
if (t < 0 || t > maxd + r) return −1.0          (0xbf800000)
p = v − dir·t;  d2 = mode ? p·p : p.x² + p.z²   (mode 0 = VMUL.xz)
R = r + t·slope
if (d2 > R·R) return −1.0
return t + 16.0·d2                              (16.0 = 0x41800000)
```
- The acquisition passes mode 1 (3-D). A lower score is better.
- The cone is small-angle: its radius grows linearly with t, at a slope equal to the angle in radians, on top of the
  target's radius.
- For the rifle (slope 0.01745 and a soldier's r ≈ 1.42 m), R is ~1.6 m at 10 m, 2.1 m at 40 m and 2.5 m at 64 m.
- The vehicle-entry cone is the third caller (B1:119544: slope 1.0, 5 m).

### 3.5 Relation (FUN_00221788) and the team table [V][O]
- rel = teamObj(self+0xd8).rel[other+0xd8]: the s8 at teamObj+0x88+team, where teamObj = *(DAT_00518388 + 0x18 +
  4·team) (B1:135680-135681).
- A second read, c2, uses other+0xdc, the object's **PerceivedTeam** (fnv 0x52a9ef0f, written by the Entity property
  setter FUN_00221420, B1:135515-135525) (B1:135682). The result is:
  - c1 when c1 ≥ 1;
  - otherwise c2, except that it reverts to c1 when c1 < c2, both objects' vtbl+0x64 → +0xe0 are set, and
    FUN_00235758 returns non-zero (B1:135684-135699).
  - The comma expression at B1:135684 assigns c2 before the comparison, so c1 ≤ 0 with c2 ≤ c1 also returns c2. The
    verifier caught this at 0x2217e8/ec.
  - [V-RAM] +0xd8 = +0xdc on all 1,141 soldier records, so this never changes a soldier's relation. What
    PerceivedTeam is for is [H].
- Encoding: −1 enemy, 0 neutral, 1 friend, so the filter's bit rel+1 makes TargetEnemy bit 0. The Lua setters write
  it: SetTeamAsEnemy 0xff, SetTeamAsNeutral 0, SetTeamAsFriend 1 (B1:27058 / 27073 / 27088; bindings
  0x516770-0x516784).
- Defaults (team ctor FUN_002acf40, 8 teams built by FUN_002ab7f8):
  - a nonzero team is friend to itself and enemy to every other nonzero team;
  - every team is neutral to team 0, and team 0 is neutral to all.
- Missions change the table from Lua. [V-RAM] the KAM1 table is exactly the default; KAS2's adds friends 2↔3 (the
  Wookiees).

### 3.6 Eligible, Bounds and the aim point, for a soldier [V][V-RAM][O]
- **+0xac** holds the kind in bits 0-2 and "alive" in bit 3.
  - The kind comes from class+0x48 & 7, copied by FUN_00221200 (B1:135467). Soldier classes store 0; [V-RAM] CIS battle
    droids are kind 2.
  - [V-RAM] Bit 3 is 1 on all 1,077 live soldier records and 0 on all 64 dead ones, over 23 BF1 states, with no
    exception.
  - The soldier's kill function FUN_001ffec0 clears bit 3 (B1:112138; it is reached through the thunks FUN_00210c30 and
    FUN_00211200).
  - The soldier's setter was not located [H]. The only `ori 8` → `sw 0xac` instructions are in FUN_001c00c8
    (0x1c010c/0x1c0118) and FUN_00213aa0 (0x213c6c/0x213c74); the network unpackers write `(x & 1) << 3`.
- **+0xe0** = (MaxHealth < FLT_MAX), set by FUN_00221200. [V-RAM] it is 1 on every soldier.
- **vtbl+0x90** for a soldier is the base FUN_00221780, `return 1` (vtable 0x532d50 + 0x90 = 0x532de0 → 0x221780)
  [V-instr].
- **Bounds** (kind ≠ 6) use the model box at vtbl+0x38()+0x1c (for a soldier, *(O+0x240)), with min at +0x80 and max
  at +0x90, local (B1:180516-180553).
  - ctr = M·((min + max)/2), where M = vtbl+0x40() (for a soldier, O+0x200: rows right, up, fwd, pos).
  - r = |max − min| / 2.
  - [V-RAM] One box is (−0.929, −0.001, −0.196)..(0.929, 2.099, 0.204): ctr is about 1.05 m above the feet and r =
    1.417. The seven box shapes seen give r = 1.387 to 1.704. The box does not change with stance.
  - The port's bake computes the model box already (`bake::soldier::BakedSoldier::aabb`, "the modl vis box"; the
    stormtrooper's x is ±0.929476). That it equals the RAM box is assumed from the matching x extent [D].
  - Kind 6 (IsA DAT_00586870, the destroyed-building kind) takes a collision prim's centre and radius instead.
- **Aim point** (vtbl+0x58 / +0x5c): the soldier's +0x58 returns −1 (0x20c678).
  - +0x5c returns M · table[O+0xbe0], with table 0x586350 = (0, 1.4, 0) / (0, 0.6, 0) / (0, 0.2, 0) for stance 0 / 1 / 2
    (FUN_0020c680 [V-instr]; the initialiser FUN_00211290 writes 0x3fb33333, 0x3f19999a and 0x3e4ccccd at
    B1:124342-124360).
  - The LOS ray stops r short of that point, on the side toward the shooter.
- **Self-box skip**: a candidate is ignored when its ctr, taken into the shooter's model frame, lies inside the
  shooter's own model box.

### 3.7 Outputs [V][O]
- **W+0x128 / +0x12c / +0x130** is the magnet. FUN_0025fea8 reads it the next frame (§5), and so does the launcher
  dwell lock.
- **K+0x30+8·ch / +0x34** is the aimed-at object: the best-scored eligible, live object in the cone out to ExtremeRange,
  of any team.
- **K+0x40+4·ch** is its relation.
  - The HUD reticle FUN_002d4b50 walks the same K's two channels (K vtbl+0x38/+0x3c) and colours by this value
    (B1:262194-262201): < 0 → DAT_0069c448, > 0 → DAT_0069c450, otherwise 0xfff2a743.
  - [V-RAM] the colours: enemy (200, 100, 100), friend (100, 200, 100), default (67, 167, 242). This is the red / green /
    blue reticle ps2live reads.

### 3.8 BF1 data (disc census over every BF1 LVL: `engine/artifacts/lead_tmp/aimf/propcensus.py`) [V]
| weapons (all sides) | LockOnRange | LockOnAngle | ExtremeRange in effect | TargetVehicle |
|---|---|---|---|---|
| rifle, pistol, shotgun, bowcaster, remote-droid blaster, wrist blaster / rocket | 64 | 1° | the bolt's Velocity × LifeSpan (rifle 120) | 0 |
| repeater / wrist shrapnel / blunderbuss | 80 / 24 / 30 | — | bolt reach | 0 |
| sniper rifle, mortar | 0 (no magnet) | — | — | 0 |
| thermal detonator / crack grenade | 30 | — | — | 0 / 1 |
| fusion cutter (TargetFriendly 1) | 3 or 6 | — | — | 1 |
| torpedo launcher | 175 | 6° (RAM 0.1047) | 320 (RAM) | 1, LockOffAngle 10 |

No BF1 class writes TargetAll or the kind-5/6 bits (hashes 0x5136c56b, 0x3f76ae9e and 0xe4aefab9 are on no class in
any BF1 LVL). Only the three mortar launchers write ExtremeRange, with the lines 64.0 then 48. **So the infantry
blasters magnet only onto enemy persons, animals and droids, never onto vehicles** (this qualifies `re-vehicles.md`
§9's "by default").

### 3.9 Checks, the RAM evidence, what stays [H]
- **Check 1 (`re_check.py fn`)**: every B1/B2 line cited in §3 lies in the function named (53 lines in FUN_0025e098;
  the rest in FUN_00241b98, FUN_00221788, FUN_002864f0, FUN_00285a98, FUN_00221200, FUN_00211290, FUN_001ffec0,
  FUN_00213aa0, FUN_001c00c8, SetTeamAs* and B2 FUN_002b0548 / FUN_002f7b28). The output is in
  `engine/artifacts/lead_tmp/aimf/checks.txt`.
- **Check 2 (absence)**:
  - `absent` finds 0x8474ea2c only in the weapon setters (B1 FUN_002864f0:210249, B2 FUN_00328fb0:320529).
  - `bitset_scan.py bf1 0xac 0x8` finds the only two `ori 8` → `sw 0xac` sites.
  - `propcensus.py` finds no class using TargetAll or the kind-5/6 bits, and LockOffAngle > 0 only on launchers and
    the K-wing missile.
  - The order inside a layer list is not searched [H].
- **Check 3 (the twin)**:
  - BF2 FUN_002b0548 is the same loop. Its vtable 0x6219fc holds it at 0x621a14, and the `PlayerController` string
    sits next to that vtable [D].
  - BF2 offsets: W+0x10c/+0x110/+0x114, K+0x180/+0x190/+0x1a0; the query uses mask 0x3b and the LOS mask 0x90
    (B2:236417-236444).
  - Its cone score FUN_002f7b28 is the same function (B2:285607-285658).
  - BF2 differs in five ways:
    0. Its widening is gated by bit 0 of K+0x38+4·ch, not by an s8 counter (B2:236219-236220).
    1. It computes a second score with a fixed 0.25 slope, which picks the HUD object (+0x190) apart from the magnet
       candidate (B2:236570-236572, 236656).
    2. A cloaked enemy (class DAT_00689828 with +0x7e3 > 0) is skipped.
    3. It skips the vehicle the shooter rides, where BF1 uses the self-box test.
    4. It adds a class IsA prefilter.
  - The port keeps BF1 magnetism on foot everywhere (PLAN §Vehicles & space), so these are not ported.
- **Check 4 (bytes)**: the ELF words are π/4 0x3f490fdb (@0x25f4a8/ac), 16.0 (@0x241c60), −1.0 (@0x241c78), 0.5
  (@0x25f36c, @0x25f8cc) and FLT_MAX (0x5384fc); ExtremeRange's lui/ori at 0x2868f4; the Lua binding table
  0x516770-0x516784; the soldier vtable slot 0x532de0 → 0x221780.
- **Check 5 (defaults and writers)**:
  - the class ctor's +0xf4 / +0xf8 / +0xd0-dc;
  - the stance table (DAT_00586354 / 64 / 74, one writer each, FUN_00211290);
  - DAT_00518388 (the team manager, set only by FUN_002ab728 / 7a8 / 7c8 / 7e0);
  - the relation bytes (the team ctor and SetTeamAs*);
  - the fire counter (written only in FUN_0025e098's counter block).
- **RAM** (23 BF1 in-level `ref/states/*.p2s`; tools `lead_tmp/aimf/{flags_ram,magnet_ram,cone_ram}.py`):
  - bit 3 matches alive, 1,141/1,141;
  - C+0xd0 / 0xd4 / 0xd8 = 120 / 64 / 0.01745 on the player's rifle;
  - the team tables are as §3.5.
  - The model reproduces every soldier HUD pick:
    - KAS2 Dock Two picks a friend at t = 106.9 m, perp 2.08 m, cone R 3.41 m. It is the only live soldier inside the
      model's cone, and it lies beyond LockOnRange but inside ExtremeRange + r.
    - 19 states have no pick, and the model finds no live soldier in the cone in any of them. Among them, BES1 (t =
      132.5, perp 3.16 < R 3.73) and KAS2 Beach Head (t = 140.6) show soldiers inside the angle but past
      ExtremeRange + r, and KAM1 post 2 (t = 9.3, perp 2.05 > R 1.55) one outside it.
    - The other 3 picks are not soldiers: a friendly building (GEO1, kind 4), a friendly vehicle (HOT1, kind 3) and a
      neutral prop (YAV1, kind 4, team 0). Each shows rel 1 / 1 / 0 with W+0x128 = 0.
    - **The per-channel bracket** (KAS2 Beach Head): the same friend at t = 140.6 m is NOT picked on channel 0 (the
      rifle, ExtremeRange 120) but IS picked on channel 1 (the pistol, ExtremeRange 150, 50 × 3.0).
  - No state catches the player magneted to an enemy (all are spawn frames). A live probe that logs it
    (`lead_tmp/aimf/probe_live.py`, ps2live's bot plus a read-only sampler) was written but not run this session: holding
    ci.lock beside the other lane was refused as interfering with its workload.
- **Verifier**: one Opus agent in the foreground, given `lead_tmp/aimf/verify/claims.md` only (32 claims A1-F3 and two
  consequences), with verdicts in `lead_tmp/aimf/verify/VERDICTS.md`.
  - Result: **30 SUPPORTED, 2 PARTLY, 0 NOT SUPPORTED.**
  - The PARTLY claims were B4 (the bolt's ordnance class is OrdnanceLaserClass 0x53a4f0, not 0x539b90; the formula
    holds) and D2 (the relation rule's c2 branch). Both are folded into §3.2 and §3.5 and reproduced by the lead.
  - The consequences: X1 SUPPORTED, with the PerceivedTeam and MountedTurret caveats; X2 SUPPORTED, with ties going to
    the first found.
  - Its own absence searches (B6, and D8 with a wider store scan) found nothing new.
- **Stays [H]**: which path sets a soldier's bit 3; the order inside a layer list; the crouch and prone aim origin;
  what PerceivedTeam is for; what the kind 5 objects are.

### 3.10 MountedTurret — FUN_00223850 (the former §3, which the port copied by mistake)

> **CORRECTION (VEH0, loop lane 2, 2026-09-25).** FUN_00223850 is the **MountedTurret's** tick (a vehicle or building
> gunner seat), not the soldier's. Its only caller is the thunk FUN_00226860 (B1:139310), whose one data reference is
> the vtable slot 0x5342fc; that vtable's typeinfo function FUN_00226ed0 references `13MountedTurret` (0x534578). The
> soldier's tick is FUN_00205cc8, and the soldier's magnet is §3.1-§3.9. For a human gunner, which of the two writers
> (this ray, or the PlayerController loop driving the turret's own Controllable) wins each frame was not traced [H].
>
> **CORRECTION (VTAT, loop lane 2, 2026-10-07; `re-turret-ai.md` 5, verifier-SUPPORTED).** The gate written below as
> `*(entity+0x1150) == 0` is the MountedTurret CLASS's ODF flag `Controllable` (K+0x1150, class default 1; BF2
> K+0x845), and `this[0x3e] < 0` is the seat's player index: no human in it. So this ray runs only for a seat no
> human drives whose class says `Controllable = 0` - on the discs the Naboo fambaa's two classes (`NAB1_H.LVL`) and
> nothing else, in either game. With the default the tick only keeps and validates a target somebody else assigned:
> an AI gunner's by its agent (`FUN_0023d1f8`), a human's by the PlayerController's scan. The same tick's tail holds
> the trigger of a weapon whose section says `AutoFire = 1` (again only the fambaa).

Runs when `this[0x3e] < 0` (no externally assigned target — AI path assigns via
`this[0xb2]`/FUN_0023d1f8 instead) and `*(entity+0x1150) == 0`:

```
weapon = this[this[0xe0] + 0xdc]              // current weapon slot
range  = weaponClass->LockOnRange             // *(weapon+0x64) + 0xd4
if (range <= 0) → keep/validate old target only (no acquisition)
frac, hitObj = RayQuery(range, aimPos=this+0x110(!row this[0x44]),
                        aimDir=this[0x48], excl=self, mask=0x5e)   // FUN_00161d68
target = hitObj ? hitObj->collision->owner (vtbl(+0xc0)[0x0d]) : 0
weapon+0x128 = target
weapon+0x12c = target ? target+0xb4 (unique id) : 0                // stale-ptr guard
weapon+0x130 = 0xffffffff                                          // aim-point hint reset
```

Notes:
- This turret path does **not** apply the TargetEnemy/TargetPerson ODF filters:
  whatever the ray hits in mask 0x5e becomes the target. *(This was written
  2026-08-30 about the "player path". It holds only for the turret seat; the
  soldier's loop filters, §3.3.)*
- Every consumer re-validates `target->0xb4 == weapon+0x12c` and clears both
  fields on mismatch (dead/reused entity guard).
- The generic reticle ray (else-branch) uses length **512.0** and mask **0x4e**
  — that one is for reticle placement, not targeting.

## 4. WeaponClass ODF properties (parser FUN_002864f0, hash = FNV-1a-32 with `c|0x20`)

| ODF property | hash | class offset | transform / default |
|---|---|---|---|
| LockOnRange | 0x7f012f70 | +0xd4 | raw float; after parse, clamps +0xd0 up to ≥ +0xd4 |
| **ExtremeRange** | 0x8474ea2c | +0xd0 | raw float, kept ≥ LockOnRange. `OrdnanceName` overwrites it with the ordnance's reach, unclamped (bolt: Velocity × LifeSpan). It is the acquisition's query and cone length (§3.2). Name resolved 2026-09-28 |
| LockOnAngle | 0x3935c7ca | +0xd8 | × 0.017453294 (deg→rad) |
| AutoAimSize | 0x7dbe88fa | +0xd8 | × **0.072** (alternative encoding of the same lock cone; 1 unit = 0.072 rad ≈ 4.125°) |
| LockOffAngle | 0xc915b156 | +0xdc | × 0.017453294 (HUD lock ellipse size) |
| MinRange | 0x1ef25b6c | +0xe4 | raw |
| OptimalRange | 0x778f9aae | +0xe8 | raw |
| MaxRange | 0x43bcebe6 | +0xec | raw; also zeroes +0xf0 |
| TargetEnemy / TargetNeutral / TargetFriendly | 0xe10d2224 / 0x5f0ae167 / 0xca7f5887 | +0xf4 bits 0/1/2 | bool flags (AI targeting) |
| TargetPerson / TargetAnimal / TargetDroid / TargetVehicle / TargetBuilding | 0xfae6054d / 0x75c58748 / 0x19007f1a / 0x281b9338 / 0xc4409ff0 | +0xf8 bits 0/1/2/3/4 | bool flags |
| *(unknown type flags)* | 0x3f76ae9e / 0xe4aefab9 | +0xf8 bits 5/6 | bool flags (names unresolved) |
| TargetAll | 0x5136c56b | +0xf8 = 0xffffffff or 0 | |
| ZoomMin / ZoomMax / ZoomRate | 0x1881b1a6 / 0x2a96e8b4 / 0x9573fb20 | +0x78 / +0x7c / +0x80 | 0 coerced to 1.0; copies to +0x84/+0x88 |

The assist math (§5) consumes **only LockOnRange (+0xd4)**. The acquisition
(§3) also reads ExtremeRange (+0xd0), LockOnAngle (+0xd8, the cone), LockOffAngle
(+0xdc, the hold cone of lock weapons) and the Target* bits (+0xf4 / +0xf8).
*(Corrected 2026-09-28. This used to say LockOnAngle is not in the passive path.)*

Soldier-class fields used (parser FUN_00225f08): `MaxTurnSpeed` (0x5433f589)
and `TurnRate` (0x0f77c4e8) both → soldierClass+0x113c; `PitchTurnFactor`
(0x0b2fc2f9) → +0x1110; `PitchLimits` (0x3403b139) → +0x112c/+0x1130 (deg→rad).

## 5. The core algorithm (FUN_0025fea8) — exact math

Called from FUN_0025e098 as
`FUN_0025fea8(dt, ctrl, remoteIdx, &axis1, &axis0, &axis2, &axis3)` where the
input struct (`ctrl[9]`) floats f[0..3] map to entity fields after the call:
f0→entity+0x64, f1→entity+0x60, f2→entity+0x68, f3→entity+0x6c. The assist
reads |f0| and modifies **f2** (yaw) and **f3** (pitch, slowdown only).
Axis identity (f0 = move stick magnitude source, f2 = aim yaw, f3 = aim pitch)
is **[hypothesized]** from the geometry (corrections are lateral-axis only);
everything else below is **[verified]**.

```
// -------- bail-outs --------
if (remoteIdx != 0) return                        // local player only
weapon = CurrentWeapon(entity)                    // FUN_0023d708; null → return
target = weapon+0x128, validated vs weapon+0x12c  // null/stale → return

// -------- frame ------------
body    = entity->vtbl294[0x06]()                 // physical body object
w       = 1.0 - fabsf(f2)                         // assist weight from yaw stick
f2 *= 0.5;  f3 *= 0.5                             // STICKY SLOWDOWN (always)
if (w <= 0) return                                // full-stick yaw: slowdown only

R = entity->vtbl294[0x0c]()                       // max turn rate [hypothesized:
                                                  //  soldierClass+0x113c MaxTurnSpeed]
                                                  // 2026-10-22 [V] (card LOOKR, re-look-ramp.md 5): the slot
                                                  //  is MaxPitchSpeed (class +0x1844; BF2 +0x758), not MaxTurnSpeed
if (entity+0x20 != 0) R /= weapon+0xdc            // zoomed: divide by zoom factor

fwd  = normalize(entity+0x110)                    // aim direction
lat  = normalize(cross(up, fwd))                  // up = *(vtbl294[0x0a]() + 0x10)
up2  = cross(fwd, lat)
pos  = row3 of FUN_001ac6f8(lat, up2, fwd, entityPos)   // player position

// -------- term 1: velocity tracking (always) --------
relVel  = body.velocity - target.velocity          // vtbl[0x12] on both
latVel  = dot(relVel, lat)
tgtCtr  = target collision-prim center             // vtbl[0x1b] prim, FUN_001645a8
fwdDist = dot(tgtCtr - pos, fwd)
track   = (fabsf(f0) * w * 0.75 * latVel) / (fwdDist * R)
f2 += clamp(track, -1.0, 1.0)

// -------- gating for term 2 --------
optionOn = profile+0x34 != 0                       // Auto Aim menu toggle
isNet    = DAT_005e9a70 ? DAT_00518be8 : DAT_00518be4
mpOn     = DAT_00518bdc != 0                       // host "aimassist" setting
armed    = (isNet && mpOn) || (!isNet && optionOn)
if (armed && *(s8*)(entity + 0x48 + weaponIdx*2) >= 1)   // fire held
    ctrl+0x30 = 1.0                                // magnet timer (seconds)
t = ctrl+0x30
if (t > 0) {
    // -------- term 2: fire magnetism --------
    pull = w * t * 0.75
    radii   = targetPrimRadius + selfPrimRadius    // prim+0xc each
    range   = weaponClass+0xd4                     // LockOnRange
    fwdDist = dot(tgtCtr - pos, fwd)               // recomputed
    if (radii < fwdDist && radii < range)
        pull *= clamp(1.0 - (fwdDist - radii)/(range - radii), 0.0, 1.0)
    if (pull > 0) {
        aimPt = target->vtbl[0x17](pos, fwd, weapon+0x130)   // soft aim point
        ok, leadPt = BallisticLead(weapon+0x6c, aimPt, target.velocity)
                                                    // FUN_0026d3f0: ordnance
                                                    // speed & gravity intercept
        if (ok) {
            dirN = normalize(leadPt - pos)
            f2  -= pull * clamp(dot(dirN, lat) / (R * dt), -1.0, 1.0)
        }
    }
    ctrl+0x30 -= dt                                 // timer decay
}
```

Exact constants (all from FUN_0025fea8 @ 0x0025fea8 unless noted):
- `0.5` — slowdown factor on both aim axes.
- `0.75` — tracking gain (term 1) **and** magnetism gain (term 2).
- `1.0` — magnet timer refill (seconds of pull after releasing fire).
- clamps: track term and pull direction term each clamped to [-1.0, 1.0];
  distance fade clamped to [0.0, 1.0].
- masks: the soldier's acquisition query 0x1e and its LOS 0x48 (§3). The
  MountedTurret's ray 0x5e and its reticle ray 0x4e / 512.0 are FUN_00223850's
  (§3.10).
- `LockOnRange` (weaponClass+0xd4) — ODF-loaded per weapon, ≤0 disables.

Interpretation (for the port; follows algebraically from the code):
- Term 1 in stick units equals `angularVelocity_lateral / R` — downstream the
  game computes `yawRate = stick * R`, so term 1 rotates the aim at exactly
  the target's apparent lateral angular velocity, scaled by
  `|move| * (1-|yawStick|) * 0.75`. No dt appears: it is a rate match, not a
  positional step → exponential convergence, frame-rate independent.
- Term 2 equals `pull * angleError / (R*dt)` in stick units → a positional
  step that closes `pull` (max 0.75) of the remaining lateral angle *per
  frame*, but the clamp caps it at one full-stick frame (`R*dt` radians), so
  large errors close at max turn rate and the last `R*dt` radians snap by
  fraction `pull`.
- The pull aims at the **ballistic lead point**, not the target itself: the
  magnetism builds in target leading for slow projectiles (FUN_0026d3f0 solves
  `|d + v·t| = s·t` for flat fire and adds a gravity-drop correction
  otherwise, using OrdnanceClass speed vtbl+0x2c / gravity vtbl+0x30).

## 6. Option & session plumbing

- Profile blob: `0x00643718 + profileIdx*0x2e4`; **+0x34 = Auto Aim flag**
  (int 0/1). Read via FUN_0034d638 (current profile) in the gameplay path and
  FUN_0034d798 (by index, `DAT_005187fc`) in the shell bindings.
- SP: Auto Aim toggle gates only term 2 (fire magnetism).
- MP: `ScriptCB_SetNetGameDefaults` arg 4 → `DAT_00518bdc`; gates term 2 for
  everyone; the local Auto Aim option is ignored in net games.
- Terms 1 and the 0.5 slowdown are **not gated by anything** except having a
  magnet target — i.e. even with Auto Aim OFF, BF1 still slows and tracks.
  (Worth a PCSX2 sanity check; it is unambiguous in the decomp.)

## 7. Reference-trace test vectors

Symbolic; plug in `R` (MaxPitchSpeed; corrected 2026-10-22, RE_LOG; was MaxTurnSpeed) and `dt` from the trace itself. All
computable by hand from §5. Inputs are post-deadzone stick values in [-1,1].

**T1 — slowdown only.** Target valid; f=(0, 0, 0.4, 0.6); velocities zero;
fire released; timer=0.
→ f2' = 0.4·0.5 + clamp(0·…) = **0.2**; f3' = **0.3**; timer stays 0.

**T2 — full-stick bypass.** Same but f2 = 1.0 → w = 0.
→ f2' = **0.5**, f3' halved, no tracking, no pull, early return after slowdown.

**T3 — strafe tracking.** f=(1, 0, 0, 0); self vel = 5·lat m/s, target
stationary; prim-center forward distance 20 m; R = 5.
→ track = (1·1·0.75·5)/(20·5) = **0.0375**; f2' = 0.0375. (Doubling strafe
speed doubles it; doubling distance halves it.)

**T4 — fire magnetism.** SP, Auto Aim ON, fire held (timer→1), f=(0,0,0,0);
fwdDist 20, radii 1.0, LockOnRange 50, lead point 0.1 rad-sine lateral
(dot(dirN,lat)=0.1), R=5, dt=1/30.
→ pull = 1·1·0.75·(1−19/49) = 0.75·0.612245 = 0.459184;
 stick = clamp(0.1/(5/30)) = 0.6;
 f2' = **−0.27551**; timer' = 1 − dt.

**T5 — falloff edge.** As T4 but fwdDist = 50 (= range) → fade = 0 → no pull;
timer still decrements (decrement is outside the pull>0 check but inside t>0).

**T6 — option off.** As T4 with Auto Aim OFF and timer 0 → no pull, but T1/T3
effects still present.

Signs: both terms push f2 in the same direction for a given target side
(term 1 `+dot(selfVel−tgtVel, lat)`, term 2 `−dot(dirToLead, lat)`); the
absolute sign convention of "positive yaw stick" vs world CCW must be lifted
from the trace.

## 8. Open questions / PCSX2 trace plan

1. ~~**Mask 0x5e semantics**~~ **ANSWERED 2026-09-28 (§3):** the soldier's
   acquisition filters by team and kind, so a friend is never the magnet. A
   corpse is not a candidate either (bit 3 is clear when dead). Seen in RAM:
   three states hold a friendly reticle pick with `weapon+0x128` = 0. Still
   open: a live frame with an ENEMY magneted (`lead_tmp/aimf/probe_live.py` is
   ready).
2. ~~**vtbl294[0x0c] turn-rate getter**~~ **ANSWERED 2026-10-22 (`re-look-ramp.md`):** the aim
   assist's R is MaxPitchSpeed (class +0x1844; BF2 +0x758) in both games, not MaxTurnSpeed.
   Struck text: ~~confirm it returns soldierClass+0x113c
   (`MaxTurnSpeed`) and its units (rad/s vs deg/s). Trace: break on
   FUN_0025fea8+entry, read the f-register after the vcall; compare against
   the ODF value of the soldier being played.~~
3. **Axis identity** — confirm f0=move (which of strafe/forward?), f2=yaw,
   f3=pitch by wiggling one stick axis at a time and watching
   entity+0x60/64/68/6c.
4. **entity+0x20 / weapon+0xdc** — confirm zoom flag and zoom factor
   (ZoomMin/Max live at class+0x78/0x7c). Trace while scoped.
5. **entity+0x1150** — the flag that freezes target re-acquisition. It is the
   MountedTurret seat's (§3.10), not the soldier's; the soldier's loop has no
   such gate.
6. **Unknown ODF keys** — class+0xd0 is **ExtremeRange** (resolved 2026-09-28,
   §3.2). class+0xf8 bits 5/6 (0x3f76ae9e, 0xe4aefab9) are still unnamed; no BF1
   class writes them.
7. **Aim origin** — ANSWERED 2026-09-28 (§3.2): the Controllable's +0x00 is the
   aim origin (for a soldier, O+0x2a0), measured 2.104 m above the feet when
   standing. +0x110 is the aim direction. Crouch and prone follow from the rule in §10.2 step 4 (their camera
   sections), answered 2026-10-02.
8. **BF2 comparison** — the acquisition twin is B2 FUN_002b0548 (§3.9, check 3).
   It is the same loop plus a 0.25-slope HUD pick, a cloak skip and an
   own-vehicle skip. The BF2 assist core (FUN_0025fea8's twin) is still not
   compared.

## 9. Rust port notes

- Implement as a pure function
  `assist(dt, sticks[4], state) -> (sticks', timer')` with `state` =
  {target opt, positions, velocities, prim radii, aim dir, up, R, zoom,
  lock_on_range, ordnance speed/gravity, fire_held, armed} — directly
  testable against §7 vectors and PCSX2 traces.
- Cite constants as: 0.5/0.75/1.0 + clamps ← FUN_0025fea8; range/falloff ←
  FUN_0025fea8 + class+0xd4 ← … acquisition ← §3
  (FUN_0025e098 + FUN_00241b98, 16.0, π/4); lead solver ← FUN_0026d3f0.
- **Acquisition (card AIMF, 2026-09-28), what the port needs from §3:**
  1. A per-weapon-class `lock_on_angle` (rad), `extreme_range` (the ODF's
     ExtremeRange, else the bolt's Velocity × LifeSpan when OrdnanceName comes
     after LockOnRange, then kept ≥ LockOnRange only where the PS2 clamps it),
     and `target_team` / `target_kind` bits. Defaults: enemy only; kinds
     person, animal, droid, vehicle and building.
  2. A per-unit acquisition box: the soldier model's vis box, with ctr = feet +
     box centre and r = half the box diagonal (the bake has `BakedSoldier::aabb`).
  3. The PS2's aim origin (2.104 m above the feet standing) and the crosshair
     direction, for pos and dir. *(The rule behind the 2.104: §10.2 step 4.)*
  4. Candidates are live units whose kind bit passes and that are not inside
     the shooter's own box. They are scored by §3.4 (mode 3-D) and given the
     tournament rule of §3.3 in a fixed, deterministic order (the port's unit
     order; the PS2's list order is [H]).
  5. LOS is `core::perception::los` (terrain + the ordnance layer, what the
     PS2's mask 0x48 with the ordnance flag tests). It ends r short of the
     stance aim point (1.4 / 0.6 / 0.2 m).
  6. The sticky cone widens by π/4 while that channel's fire is held.
  7. The magnet is stored only when it passes the filter and lies within
     LockOnRange + r.
  8. Order: the assist runs on last tick's magnet, then the acquisition runs.
  9. The relation is the port's own (same team = friend, else enemy). That is
     the PS2's default table for two armies; the port has no neutral team.
- The lead solver must run in f32 with the same operation order (vrsqrt-based
  normalize) if bit-exact traces are wanted; otherwise document tolerance.
- OURS (playtest C5): a scoped sniper rifle on a controller matches half its
  magnet's sideways motion before the press (`aim_assist::SNIPER_FOLLOW`,
  under Auto Aim). Nothing on either disc to copy: their snipers have no
  magnet (§3.8).

## 10. The aim ray, the camera's tilt and where a shot goes — OPUS RE (loop lane 3, 2026-10-02, card AIMA) - [O] UNAUDITED

The maintainer, 2026-10-02: "the original ps2 games had the marker for where the bullet goes higher up, so the camera could be
angled up higher and looking down without the bullets going straight into the ground". This section is the rule behind
that, for both games. In one sentence: **the chase camera looks `TiltValue` below the aim direction (BF1 5°, BF2 10°),
the aim ray leaves the camera's own position along the aim direction, the reticle is drawn where that ray goes on
screen, and the shot leaves the soldier's fire point toward the first thing that ray hits.**

Conventions as §3: B1 / B2 are lines of the two decomps; O is the BF1 soldier and K = O + 0x2a0 its Controllable; E is
the BF2 soldier. M is the soldier's world matrix (B1 O+0x200, B2 E+0x120): row 0 points to the soldier's LEFT
[V-RAM: row 0 · the camera's right row = −1.000], row 1 up, row 2 forward, row 3 the feet. Tags: [V] read; [V-instr]
read from the instructions (`lead_tmp/aima/fun_00204748.dis`, `fun_0023ecb8.dis`); [V-RAM] read from PCSX2 save
states; [M] measured on a live PCSX2 session; [D] derived; [H] open. [O] marks what the checks and the verifier
covered (§10.9).

### 10.1 The camera section — the data [V][V-RAM][O]
- A soldier class holds one camera section per stance and zoom: B1 class + 0x1920 + stance·0xc0 + zoom·0x60 (the
  CameraSection key picks the slot, then FUN_0020f2e0 hands the keys to the section setter, B1:123099-123101); B2 class
  + 0x8e0 + index·0xc0 + zoom·0x60 (B2:157277-157278), where the index is the stance (0 stand, 1 crouch, 2 prone) or 3
  while E+0x7e4 == 3 (B2:157238-157241; RAM: slot 3 holds the SPRINT section).
  - BF1's CameraSection values (B1:123220-123307, hashes matched): stand, standzoom, crouch, crouchzoom, prone,
    pronezoom, and splitstand … splitpronezoom. A `split*` section goes into the same slot, and its keys are applied
    only while the camera manager has more than one viewport (B1:123099).
- The section setter is B1 FUN_001b6168 (B1:59751-59845), B2 FUN_001d0328 (B2:79429-79523), identical:

  | key | hash | section offset | notes |
  |---|---|---|---|
  | EyePointOffset | 0x41568c97 | +0x00 (xyz) | also written to +0x10 (TrackCenter's default) |
  | TrackCenter | 0xe85d5895 | +0x10 | |
  | TrackOffset | 0xfd3d9507 | +0x20 | x right, y up, z back, in the camera's frame |
  | MoveTensionX / Y / Z | 0x88f1308a / 0x89f1321d / 0x86f12d64 | +0x40,+0x44 / +0x48,+0x4c / +0x50,+0x54 | two floats (the + and − direction); one value fills both. Names by hash search, and in the mod tools' key list (`community_mods/gametoast_lua/doc/StarWarsBattlefront/Documentation/odfguide.md`) |
  | MoveTension | 0x21050bf6 | all six | |
  | AimTension | 0x5e3171cc | +0x58 | |
  | TiltValue | 0x359d5227 | +0x5c | × 0.017453294: stored in radians (B1:59776-59779, B2:79454-79457) |

  The constructor (B1 FUN_001b56d0, B2 FUN_001ce800) sets tilt 0, the move tensions 30, AimTension 30 (B2: 1000) and a
  fixed point (0, 1.5, 0) at +0x30 that no key writes (the camera collision's origin, §10.6). BF2's soldier class
  constructor then overwrites its sections (FUN_00250488, B2:169438-169559): MoveTension 15 and AimTension FLT_MAX
  (B2:169449), which is why no BF2 ODF needs to say so.
- The tilt is read through one getter: B1 FUN_001b58c0 returns +0x5c, × `DAT_005174d8` = 0.4 when `DAT_00519078` >= 2;
  B2 FUN_001ce9b8 returns +0x5c × 0.4 when `*(DAT_005fc4ec + 0x20)` > 1 [V-instr both]. `DAT_00519078` is the
  split-screen count: 1 in the ELF and in every state; ScriptCB_SetSplitscreen writes 2 or 1 (B1:19346-19348) and the
  `/splitscreen` start-up switch 2 (B1:14175). **In split screen the tilt is 0.4 of the authored one** (BF1 2°,
  BF2 4°).
- [V-RAM] The values in the player's class (`lead_tmp/aima/limits.txt`; 5 BF1 and 6 BF2 classes, all alike but the BF1
  snowtrooper's zoom):

  | game, section | EyePointOffset = TrackCenter | TrackOffset | TiltValue | AimTension | MoveTension |
  |---|---|---|---|---|---|
  | BF1 STAND | (0, 1.8, 0) | (0, 0, 3.5) | 5° | 30 | 15 |
  | BF1 CROUCH | (0, 1.3, 0) | (0, 0.15, 3.5) | 5° | 30 | 15 |
  | BF1 PRONE | (0, 0.5, 0) | (0, 0, 3.0) | 5° | 30 | 15 |
  | BF1 zoom sections | as the stance | (0.4, 0.05 / 0.2 / 0.2, 2.8); snowtrooper (0.3 / 0.4, 0, 2.5) | 3.5° (snowtrooper 2.5°) | FLT_MAX | FLT_MAX |
  | BF2 STAND | (0, 1.8, 0) | (0, 0, 3.2) | 10° | FLT_MAX | 15 |
  | BF2 CROUCH | (0, 1.3, 0) | (0, 0.15, 3.0) | 10° | FLT_MAX | 15 |
  | BF2 PRONE | (0, 0.5, 0) | (0, 0, 3.0) | 5° | FLT_MAX | 15 |
  | BF2 SPRINT (index 3) | (0, 1.8, 0) | (0, 0, 4.0) | 10° | FLT_MAX | 15 |
  | BF2 zoom sections | as the stance | (0.4, 0.05 / 0.2 / 0.2, 2.8) | 3.5° | FLT_MAX | FLT_MAX |

  A FLT_MAX tension means "at once" (the blend factor is exp(−tension·dt), §10.6).

### 10.2 The soldier's aim update — B1 FUN_00204748, B2 FUN_0023ecb8 [V][V-instr][O]
Called once per soldier update, after the stance's movement (B1:117775 in the soldier tick FUN_00205cc8; B2:161062 in
FUN_00240bf8), and once at the spawn (B1:121371 FUN_0020c880; B2:167618 FUN_0024de60). No other caller: `re_check.py
callers` and a `jal` / data-word scan agree (2 each). A dead soldier is skipped (bit 3 of O+0xac, B1:115445).

1. **Aim direction** d = M.up·sin θ + M.forward·cos θ, θ = pitch + kick (B1 O+0xa30 + O+0xa34, B1:115467-115487,
   0x204834-0x204884; B2 E+0x560 + E+0x564, B2:156992-157012). Stored at K+0x110 (B2 E+0x350). The yaw is the body's.
   - d is copied instead (from that Controllable's +0x110; B2 +0xe0) when the player's current Controllable — the
     first non-null of the three pointers of the object at O+0x380 (B2 E+0x328) — has a class whose word +0x230 (B2
     byte +0x224) is set (B1:115459-115511, 0x2047dc-0x204830). For a player on foot that word is 0 [V-RAM by the
     model: d reproduces from the pitch in all 132 states]; which classes set it was not read [H].
   - The sine and cosine come from a 2048-entry table with linear interpolation (FUN_004453a8); the camera's 5° reads
     4.9954° in RAM. Not worth copying.
2. **Fire point** F = M·T0[stance] + d·T2.z + M.row0·T2.x + (d × M.row0)·T2.y (B1:115512-115548, 0x204890-0x204930;
   B2:157035-157071 and 157215-157233). The two tables are written once by a static initialiser (B1 FUN_00211290,
   B1:124337-124365, at 0x5862f0 / 0x586320; B2 FUN_00256128, B2:172990-173007, at 0x689840 / 0x689870) and hold the
   same numbers in both games:

   | stance | T0 (x along row 0, y up, z forward) | T2 (x along row 0, y along d × row0, z along d) |
   |---|---|---|
   | stand | (0.06, 1.7, 0) | (−0.12, −0.10, 0.25) |
   | crouch | (−0.1, 1.2, 0) | (−0.12, −0.05, 0.25) |
   | prone | (0.06, 0.6, 0.4) | (−0.12, −0.20, 0.15) |

   d × row0 is the "up" of the aim (perpendicular to d in the vertical plane). So a standing soldier's fire point is
   a shoulder pivot 1.7 m up, with the gun 0.25 m ahead along the aim, 0.10 m below it and **0.06 m to his right**
   [V-RAM: every state reads 1.5828 m above the feet, 0.2424 m ahead and 0.060 m right of the camera axis at the spawn
   pitch]. It is a table, not the weapon model's hardpoint, and it serves bots and players alike.
   M·T0 is also stored at K+0x100 (B2 E+0x340) for every unit; only step 4 replaces it with the eye.
3. **Who takes the camera path**: a soldier with a player index (B1 O+0x388 = K+0xe8, B2 E+0x330) >= 0 and a view
   object (B1 O+0x558, B2 E+0x2a0) (B1:115552-115556; B2:157057, 157116). Everyone else takes step 7.
4. **Third person** (the view object's first-person flag clear, B1:115629-115649): the game rebuilds the camera it is
   about to draw, without the smoothing and the collision:
   - eye = M·EyePointOffset, stored at K+0x100 (B1:115650, 115678-115681; B2:157278, 157306-157309);
   - a frame with back = −d, right = normalize(M.up × back), up' = back × right (B1:115652-115677, 0x204b94-0x204bf4),
     turned about its own right axis by −tilt: the quaternion of the frame times (X axis·sin(−tilt/2), cos(−tilt/2))
     (B1:115699-115718; the X axis is the constant (1,0,0) at 0x66eef0, FUN_00445b28 is the Hamilton product; B2:157326-
     157345). [V-RAM] the sense: the camera's forward is `tilt` BELOW d;
   - cam = M·TrackCenter + R·TrackOffset (FUN_001b5850 / FUN_001ce948, B1:115720, B2:157346);
   - **aim origin A = cam + d·((eye − cam)·d)**, stored at K+0x00 (B1:115722-115739, 0x204d60-0x204da4; B2 E+0x3c0,
     B2:157348-157365).
   So the aim ray is the ray from the (ideal) camera position along d, and its origin is slid forward until it is
   abreast of the soldier's eye point, so nothing between the camera and the soldier can block it.
   - [D] Standing, A is above the feet by 1.8 + L·sin(tilt − θ) + L·cos(tilt)·sin θ, L = TrackOffset.z: [V-RAM] 2.1043
     m at the BF1 spawn pitch (§3.2's "2.104"), 2.085 at θ = +5°; BF2 2.354 at the spawn pitch, 2.311 at −23.1°. Crouch
     and prone follow from their sections (§8 item 7 is answered by this rule, not by a height).
5. **The ray** (third and first person alike): start = A + d·(TrackOffset.z + 2.0); fraction = Ray(length 60.0, start,
   d, exclude the soldier, mask (the 8th argument), ordnance bodies only (the 9th, 1)); **H = start + d·60·fraction**,
   stored at K+0x10 (B2 E+0x3d0).
   B1:115750-115785 with `lui 0x4000`, `lwc1 6472(v1)` (class + 0x1948 = the section's +0x28), `lui 0x4270`,
   `addiu t2, 78`, `jal 0x161d68` at 0x204dd4-0x204e40; B2:157418-157456 with `lui 0x4000` 0x23f424, `lwc1 2312(a0)`,
   `lui 0x4270` 0x23f4d0, `addiu t2, 154`, `jal 0x161c60` 0x23f4f8.
   - The mask is 0x4e in BF1 (soldier 2 | vehicle 4 | building 8 | terrain 0x40, re-collision-layers §4) and 0x9a in
     BF2 (its layer bits: terrain 0x80, building 0x10, and 8 | 2 [D by the twin]).
   - With nothing hit H is 60 m past the start [V-RAM: 41 BF1 and 5 BF2 states read 60.0; M: the sky rows of §10.5];
     the wrapper's own default is 1.0 (FUN_00161d68, B1:1735-1744).
   - So H is the first ordnance-solid thing on the reticle's ray between TrackOffset.z + 2 and TrackOffset.z + 62 m
     from A: **5.5 to 65.5 m in BF1, 5.2 to 65.2 m in BF2** (standing). Nearer than that is not seen at all.
   - BF2 only: while the session flag `DAT_00600548` / `_549` is set the ray is cast on one frame in
     max(1, (`DAT_006d85b8` + 3) / 4) and the old distance is reused between (B2:157195-157214, 157376-157416), and a
     player who is not local gets 20 m without a ray (B2:157418, 157442-157448). The flags are 0 and the count 1 in
     every BF2 state read (offline): the ray runs every frame. `_549` is set by ScriptCB_SetAmHost's helpers and by
     two session-object functions, one of them in a decomp hole (store at 0x40bc24) [verifier].
6. **The aimer**: … B2 FUN_003018b8
   (E+0x580, …) (B2:157460-157475). The setter stores the position at aimer+0x80 and +0x90 (B2 +0x80 and +0xa0), the
   direction at +0x50 and a "set from outside" flag (+0x2c; B2 byte +0x29) (B1:191127-191154); the aimer's own update
   returns at once while that flag is set (FUN_0026d748, B1:191808-191810), and FUN_0026cdf0's four callers are all in
   FUN_00204748: nothing else moves a BF1 soldier's aimer. (BF2's setter has a fifth caller, FUN_0033c9e0, not read.)
   The weapon spawns its ordnance at aimer+0x90 along the normalised aimer+0x50, plus the spread
   (`re-blaster-damage.md` §3.3). **A player's shot leaves F and flies at H.**
7. **Not the camera path**:
   - zoomed (K+0x20 ≠ 0) with a current weapon, and either FUN_00283d18(weapon) true — its class word +0x8c, the
     ODF key **ZoomFirstPerson** (0x14ec7e1d, B1:209883-209885), or an owner object test not traced — or the view in
     first person: aimer (F, d), K+0 = F, K+0x10 = F + 200·d, and K+0x100 = F (prone: M·T0 − row0·0.12)
     (B1:115557-115601). BF2's twin of this path hands the aimer M·T0 + d·T2.z, without the two sideways terms
     (0x23ee0c-0x23ee38, 0x23ef44) [verifier; reproduced];
   - first person, not zoomed: A = F (B1:115745-115748), then step 5 and 6;
   - a bot (player index < 0) or a soldier with no view object: K+0 = F, K+0x10 = F + 200·dir, aimer (F, dir), where
     dir = d, or, for a bot whose class word +0x7c is 0, d passed through the aim tracker FUN_00236700 (a spring on
     the aimer's direction; B1:115793-115831). **A bot's shot leaves the same F along its own aim; it has no tilt and
     no ray.**
   - The first-person flag is the +0x14 word of a static per-player view record (O+0x558 points at it): 0 in every
     state; written by FUN_001b72f0, whose callers include ScriptCB_SetFirstPersonView and the soldier's toggle
     FUN_00204578 [verifier; the Script function is at B1:19795]. A class can force either view (its record's +0x1b8
     = 1 or 2, B1:115634-115640).
   - A bot's class word +0x7c is 0 on the player's class in RAM; its one writer (FUN_002ae110, B1:239001) sets 1 when
     the class joins a team's list [verifier] — what that list is was not read [H].

### 10.3 Pitch [V][V-RAM][M][O]
- At the spawn the pitch is **−0.07 rad** (−4.01°) and the kick 0 (B1:111670-111674 in FUN_001ff1e8, 0xbd8f5c29;
  B2:149578-149588 in FUN_00234a68, the same word). [V-RAM] the 44 states saved at a spawn (23 BF1, 21 BF2): aim
  −4.0107°, camera −9.0107° (BF1) and −14.0107° (BF2).
- Each update (B1:117271-117345 in FUN_00205cc8; B2:160072-160107 in FUN_00240bf8): the kick rate (O+0xa38) feeds the
  kick offset (O+0xa34), which decays by min(6·dt, 1) of itself while the rate decays by min(20·dt, 1); part of the kick
  goes into the pitch itself, times min(timer O+0xa3c, 1). Then the stick's pitch rate is added and the pitch is
  clamped: ±0.4 rad while the state word is 2 (prone), else the class's PitchLimits (B1 class + 0x1848 / +0x184c,
  B2 + 0x75c / +0x760; setters B1:123038-123043, B2:171882-171887, degrees × 0.017453294).
- [V-RAM] PitchLimits = −0.8 / +1.1 rad (−45.84° / +63.03°) on all 11 player classes read. [M] the stick held down
  stops at −45.84° in both games. So **the camera looks down to −50.84° (BF1) / −55.84° (BF2) and up to +58.03° /
  +53.03°**. The port already clamps to (−0.8, 1.1) and ±0.4 prone (`core/src/soldier.rs`).

### 10.4 The reticle on screen [V][V-RAM][O]
- B1 FUN_002d4b50 (B1:262063-262085): p = K+0x00 + K+0x110·1024.0; p goes through the camera's view matrix and the
  projection (FUN_00491170); the reticle element gets **x = 0.5** and **y = projected y·0.5 + 0.5**.
- B2 FUN_0038e598 (B2:387525-387565): the same point (K+0x150 + K+0xe0·1024.0 of the Controllable = E+0x3c0, E+0x350),
  the same (0.5, y·0.5 + 0.5), published to the HUD event `player%d.weapon%d.reticule.position` (registered at
  B2:408027, FUN_003b0aa8). The loop runs for both weapon slots but halves the same value again on its second pass,
  so `weapon2` gets (0.5, 0.25·y + 0.75) — an event with no listener in the states read [verifier].
- [V-RAM] What the game itself stored (`lead_tmp/aima/stored_reticle.py`): BF1's reticle element holds (0.5,
  0.371835) = line 178.481 of 480, at the spawn pitch and at −45.3° alike; BF2's holds (256.0, 143.1256) of the 512 ×
  448 frame = line 153.349, at −4°, −23° and −45.8°.
- [D] The aim ray passes through the ideal camera position, so on screen it is one point: straight above the centre by
  tan(tilt) / tan(fov_y / 2) of the half height. **BF1: tan 5° / 0.34131 = 0.2563 → 61.5 lines of 480 above the
  centre, y = 178.5. BF2: tan 10° / 0.48838 = 0.3610 → 86.65 lines, y = 153.35.**
- [V-RAM] Projected through the live camera of each state: BF1 (320.0, 178.20-179.03) in 105 states (178.48 wherever
  the camera is not pulled in), BF2 (320.0, 153.35) in 27 states, at every pitch read (BF1 −45.8° to +7.6°, BF2 −45.8°
  to +11.3°).
- [M] The drawn BF1 reticle's box in the frame is x 294..345, y 157..200: centre (319.5, 178.5). BF2's reticle mesh is
  not symmetric about its anchor: its box is x 284..358, y 113..179 (centre y 146) around the anchor at 153.35
  (`lead_tmp/aima/frames/ps2_reticle_sheet.png`).
- [D] The offset is an ANGLE from the camera's eye, not a height on the wall: the reticle does not move with the
  distance of what it is over. It moves only when the drawn camera leaves the ideal one — pulled in by a wall (by
  0.15-2.4 m in 24 BF1 states: y 178.2-179.0) or, in BF1, lagging a turn (AimTension 30).

### 10.5 What the shot does — measured [V-RAM][M][O]
- **In flight** (save states with the player firing; `lead_tmp/aima/bolts.py`): 14 BF1 and 8 BF2 player bolts (the
  BF2 ones are each seen in two or three consecutive states).
  Every one spawned at the aimer's position F to 0.000 m. The least-spread bolts fly 0.06-0.17° from the aimer's
  direction and pass 0.03-0.05 m from H, at (318.5-320.7, 177.9-179.3) of the BF1 frame and (319.6-321.0, 152.5-154.5)
  of the BF2 frame — under the reticle. Held fire spreads the rest up to 3.8° (BF1) and 1.1° (BF2).
- **Live, one pitch at a time** (`lead_tmp/aima/probe_live.py`; single taps of FIRE, three per pitch; BF1 NAB1, a
  stormtrooper facing a grass rise; BF2 HOT1, a snowtrooper facing a snow slope). "Lands" is the bolt's last position,
  as a horizontal distance from the feet; the screen point is that position through the live camera:

  | game | aim pitch | camera pitch | A above feet | H from A | bolts pass H at | lands ahead | on screen | below the reticle |
  |---|---|---|---|---|---|---|---|---|
  | BF1 | −4.01° | −9.01° | 2.104 | 9.72 | 0.016-0.037 m | 8.8 m | (319.8, 181.0) | 2.5 lines |
  | BF1 | −11.75° | −16.75° | 2.099 | 5.97 | 0.004-0.019 m | 5.8 m | (320.4, 179.9) | 1.5 |
  | BF1 | −19.50° | −24.50° | 2.088 | 65.50 (no hit) | — | 3.30 m | (325.3, 225.8) | 47 |
  | BF1 | −28.53° | −33.53° | 2.068 | 65.50 | — | 2.45 m | (326.4, 230.8) | 52 |
  | BF1 | −36.27° | −41.27° | 2.046 | 65.50 | — | 1.85 m | (327.2, 234.4) | 56 |
  | BF1 | −45.30° | −50.30° | 2.015 | 65.50 | — | 1.43 m | (326.9, 236.2) | 58 |
  | BF1 | +7.60° | +2.60° | 2.102 | 65.50 (sky) | 0.15-0.22 m | 50-59 m on the rise, or 120 m (the bolt's life) | (318.5, 178.7) | 0.2 |
  | BF2 | −4.01° | −14.01° | 2.354 | 11.29 | 0.015-0.035 m | 11.2 m | (320.4, 153.7) | 0.4 |
  | BF2 | −11.65° | −21.65° | 2.344 | 8.01 | 0.009-0.021 m | 7.7 m | (319.9, 154.8) | 1.5 |
  | BF2 | −19.29° | −29.29° | 2.325 | 6.14 | 0.006-0.015 m | 6.2 m | (320.0, 151.8) | −1.5 |
  | BF2 | −28.84° | −38.84° | 2.287 | 65.20 (no hit) | — | 2.82 m | (324.4, 207.6) | 54 |
  | BF2 | −36.48° | −46.48° | 2.247 | 65.20 | — | 2.17 m | (325.2, 212.6) | 59 |
  | BF2 | −45.84° | −55.84° | 2.187 | 65.20 | — | 1.51 m | (325.2, 217.0) | 64 |
  | BF2 | +7.64° | −2.36° | 2.351 | 26.76 | 0.065-0.105 m | 25.4 m | (320.0, 154.7) | 1.3 |

  In all 44 single shots the bolt left F (within 0.0045 m) at 0.03-0.25° from normalize(H − F).
- What the table says:
  1. **Within the ray's reach the shot lands under the reticle** (within 2.5 lines), whatever the distance and the
     pitch: the parallax between the fire point and the reticle's ray is taken out by aiming the bolt at H.
  2. **Past the ray's reach (nothing within 65.5 / 65.2 m)** the bolt is aimed at the ray's far end, crosses the
     reticle's ray there and goes on above it. [D] F is 0.51 m (BF1) / 0.77 m (BF2) under the ray at the soldier, so
     at 120 m the bolt is 0.42 m / 0.65 m over it: 2.5 / 2.6 lines of 480 above the reticle. In practice: on it.
  3. **Nearer than the ray's start** (the ground or a wall closer than 5.5 / 5.2 m along the reticle's ray — on
     flat ground [D] any aim steeper than about −22° (BF1) / −26° (BF2)) the ray starts inside or behind the thing, finds
     nothing and reads 60 m; the bolt then flies nearly parallel to d (0.4° / 0.65° off it) from the fire point and
     lands 47-64 lines BELOW the reticle, 1.4-3.3 m in front of the feet. At the steepest aim it lands 1.43 m (BF1) /
     1.51 m (BF2) ahead of the feet, never on them.
  4. A terrain ray that starts under the ground reports no hit [M: the four BF1 and three BF2 steep rows]; the code
     path was not read.
- The same ray feeds §3's acquisition (pos = K+0, dir = K+0x110): the magnet's cone and the reticle's colour are
  centred on the reticle's ray, not on the view's centre.

### 10.6 The drawn camera — B1 FUN_0020c110 → FUN_001b58f0, B2 FUN_0024cd00 → FUN_001cef68 [V; B2 by structure]
- Third person (B1:121005-121016): FUN_001b58f0(dt, k1, k2, section, the camera's own matrix (in and out), M, d,
  soldier), k1 = 0.5 while the state word is 2, else 1; k2 = 0.1 when dead, else 1. Then the eye moves along the
  camera's back row by `DAT_005178b0` = 0.0 (no effect). First person does not call FUN_001b58f0: the frame of d at
  K+0x100 − d·0.3 (`DAT_005178b4`, B1:120931-120988). Neither global has a writer.
- FUN_001b58f0 (B1:59253-59748):
  1. the same frame and the same −tilt turn as §10.2 step 4 (B1:59359-59429);
  2. the orientation is slerped from last frame's by exp(−AimTension·k2·dt) (B1:59435-59438; the exponential is
     FUN_00435370 [D by its use]) unless the track centre jumped more than 80 m (6400.0 on the squared distance,
     B1:59356), which snaps it;
  3. eye = M·TrackCenter + R·TrackOffset (B1:59446);
  4. collision: from the point M·(0, 1.5, 0) (the section's +0x30) five rays go to the eye less half the near
     distance — the centre one and four moved by ± half the near plane's width and height along the camera's right and
     up rows — with FUN_00161cb8, mask 0x48 (building | terrain), ordnance bodies (B1:59447-59626). The smallest
     fraction goes through FUN_0019fd70 (B1:59627): a smaller fraction than the stored one is taken at once; a larger
     one is approached at 0.75·dt per update, and only once it is 1.0 or more than 0.15 above the stored one. The eye is
     then origin + (eye − origin)·fraction, moved half the near distance further out along the ray (B1:59629-59649);
  5. unless snapped, the eye's step from last frame's eye is taken into the camera's frame and each axis is scaled by
     1 − exp(−MoveTension·k1·dt), with the + or − tension by the step's sign (B1:59651-59730).
- B2 FUN_001cef68 (B2:78333-78816) has the same calls in the same order (tilt getter B2:78502, the orientation blend
  on +0x58, the eye at B2:78536, the collision ray with mask 0x98, the six tensions). During a change of stance
  FUN_001cf738 blends the old and the new section over a class time (B2:166875-166901); FUN_0023ecb8 does not, so
  for that moment the aim origin comes from the new section alone and the reticle moves on screen [D]. Not line-read
  beyond that.
- [V-RAM] The drawn camera is the ideal one to 1 mm in 20 of the 23 BF1 states not under a RAM hold and to 3.4 mm in
  all 27 BF2 states; the other three are pulled in toward the soldier (0.15, 0.28, 0.51 m).
- BF1's AimTension 30 makes the drawn view trail a turning aim; BF2's is FLT_MAX (no lag).

### 10.7 The other controllables [V callers only]
The tilt getter has nine callers per game: the camera placement, the soldier's aim update, one unread helper (B1
FUN_001b6800, B2 FUN_001ce9f0) and six more functions that pass their own class's section (B1 FUN_001c9398,
FUN_001d1fe8, FUN_001dbbd8, FUN_001eba60, FUN_0021b3a0 and the MountedTurret's FUN_00223850; B2 FUN_001ee668,
FUN_001f99d0, FUN_00209758, FUN_0021ca80, FUN_00261208, FUN_0026cea0). [H] those are the vehicles' and turrets' aim
updates, tilting their cameras the same way; their bodies were not read here (the port's seat camera is PROVISIONAL,
FEEL_NOTES VSEAT). Their collision origin is read (issue #71, `walk::vehicle_pull_height`): every controllable's camera
goes through the same FUN_001b58f0 / FUN_001cef68, from its own section's +0x30 on the frame it passes. The class setters
write that point on `GeometryName`: 0.9 of the model's vis box top for a hover or flyer, 0.8 for a walker. A droid's is
(0, 0.15, 0) from its constructor, and a MountedTurret keeps the 1.5.

### 10.8 What the port does today, and what a build needs
Today (`game/src/walk.rs` `WalkSim::camera`, `core/src/sim/step.rs`, `core/src/unit.rs`, `ui/src/hud.rs`): the camera
looks along the aim (no tilt), 3.2 m behind feet + 1.8 on both games; a shot leaves feet + 1.425 m (75 % of the
capsule) along the aim; the reticle is at the window's centre; the acquisition looks from feet + 2.104 m; the spawn
pitch is 0.
1. Camera: forward = the aim turned down by the section's tilt; eye = track centre + R·TrackOffset; per game and
   stance from §10.1's table (BF1's 3.5 m is card CAMS's finding; the per-class exceptions stay CAMS's).
2. Sim, a human on foot in third person: A and H by §10.2 steps 4-5 from the ideal camera (no smoothing, no
   collision), the bolt from F toward H. The ray needs the units' capsules, the vehicles' hulls, the ordnance layer
   and the terrain. The acquisition takes pos = A.
3. Sim, everyone: the fire point F by §10.2 step 2 in place of `muzzle_height` (a bot's shot too: this moves every
   sweep and arena hash, so it is its own card).
4. HUD: the reticle at x = centre, y = the projection of A + 1024·d through the drawn camera; with the port's Hor+
   camera that is tan(tilt) / tan(fov_y / 2) of the half height above the centre, at any aspect and any FOV setting.
5. The spawn pitch −0.07 rad; split screen: tilt × 0.4.
6. The camera collision of §10.6 step 4 against `game/src/camera_collide.rs` (PROVISIONAL today).

Since 2026-10-28 (playtest; `analysis/camera-occlusion.md`, ours): every map plays BF2's soldier section and FOV
(`core::unit::camera_section`; §10.1's table is `ps2_camera_section`), small props and vehicles are drawn
see-through instead of pulling the camera in, and a wall pulls it toward step 4's 1.5 m point.

Port deviations from §5 and §10 (2026-10-28, branch `aimfix`, `[ours]`; one line each in FEEL_NOTES): past the ray's
reach the shot converges on the reticle's pick instead of item 2's far end (`Sim::beyond_reach`), or, with no pick,
on the terrain or world a second ray meets within 305 m (2026-11-05, playtest B6, `FAR_AIM_LENGTH`; ground maps only); a human's bolt and
ray meet the soldiers 1.25 ticks back along their velocity, where the interpolated window drew them
(`core::unit::HUMAN_SHOT_LAG`); §5's pull aims at a low-passed, eased lead point and term 1 rate-matches its motion in
full whatever the move stick (`core::aim_assist::Lead`). Branch `sniperassist`: every disc sniper ships LockOnRange 0;
the class loader gives a sniper rifle (`ZoomFirstPerson` and in the AI sniper table) LockOnRange = its MaxRange (128 m),
its own LockOnAngle (1°), and TargetVehicle when its round hurts vehicles (BF2 only)
(`core::weapon::WeaponClass::give_sniper_assist`); a mouse-aimed pad is still never pulled.

### 10.9 Checks, evidence, what stays [H]
- **Check 1 (`re_check.py fn`)**: every B1 / B2 line cited in §10 lies in the function named —
  `engine/artifacts/lead_tmp/aima/checks.txt` (109 lines checked).
- **Check 2 (absence / callers)**: 0x359d5227, 0x5e3171cc and 0x41568c97 occur only in the two section setters;
  FUN_00204748 and FUN_0023ecb8 have two callers each by text and by a `jal` / data-word scan
  (`scripts/tools/refs.py`); FUN_0026cdf0's four callers are all in FUN_00204748.
- **Check 3 (the twin)**: BF2's FUN_0023ecb8 was read beside BF1's, branch by branch; its callers and its tables'
  writer are named above; the HUD and camera twins are FUN_0038e598 and FUN_001cef68.
- **Check 4 (bytes)**: 0.4 at 0x5174d8; `lui 0x4270` (60.0) 0x204df4 / 0x23f4d0; `lui 0x4000` (2.0) 0x204dd4 /
  0x23f424; masks `addiu t2, 78` 0x204e38 / `addiu t2, 154` 0x23f4f4; 200.0 `lui 0x4348` 0x204a8c, 0x204ef0, 0x204f6c;
  the table words at B1:124337-124365; 0.0 / 0.3 at 0x5178b0 / 0x5178b4.
- **Check 5 (writers)**: the stance tables have one writer each (the static initialisers); `DAT_00519078` (ELF 1) is
  written 2 or 1 by ScriptCB_SetSplitscreen and in four other functions; `DAT_005174d8`, `DAT_005178b0` and
  `DAT_005178b4` have no store (the decomp text, `lead_tmp/re/wse/addrscan.py` and the verifier's own constant scan;
  `addrscan.py` alone is not proof — it loses a `lui` held across a `jr` delay slot and so misses FUN_001b58c0's own
  read of 0x5174d8); BF2's network flags (`DAT_00600548`, `_549`, `_54f`, `_560`) have 5, 7 (+1 in a hole), 2 and 5
  writers and read 0, 0, 0, 0 in RAM offline.
- **RAM**: `lead_tmp/aima/model_bf1.py`, `model_bf2.py` re-run the update on a state and compare with what the game
  stored. 105 BF1 states (21 spawn, 82 walk, 2 other) and 27 BF2 states: d, F, eye, A and the aimer's direction
  reproduce to 0.5 mm / 1e-5 in every state not being written by a RAM hold (the 82 walk states and four BF2 states
  were saved under one: 2 cm).
- **Live**: two software-renderer sessions (BF2 HOT1, BF1 NAB1), `lead_tmp/aima/live/*.json` and a state per pitch;
  the sheet `lead_tmp/aima/frames/ps2_live_pitch_sheet.png` marks the reticle's row and the measured impact.
- **Verifier**: one Opus agent in the foreground, given `lead_tmp/aima/verify/claims.md` only (46 claims A1-H4);
  verdicts in `lead_tmp/aima/verify/VERDICTS.md`. **40 SUPPORTED, 6 PARTLY, 0 NOT SUPPORTED.** It checked every
  vector claim at instruction level (no sign or operand error; FUN_004453a8's first output is the sine), re-ran the
  RAM scripts and found the game's own stored reticle position. The PARTLY items are folded in and the lead
  reproduced each: B11 (the scope path needs a current weapon and also rewrites K+0x100), B12 (FUN_00283d18 has a
  second way to be true), B14 (the mask is the 8th argument), C1 (BF2's scope path drops the fire point's sideways
  terms; the stance for the tables is never 3), E2 (the `weapon2` value) and H4 (two rows without shots). Its
  additions: the BF1 `split*` sections, BF2's class constructor defaults, the first-person flag's record and writers,
  the network flags' defaults and the hidden store, the 8 distinct BF2 bolts.
- **Stays [H]**: which classes set the "copy the aim" word of step 1; FUN_00283d18's second test; what list sets a
  class's +0x7c; why a bolt's stored stop is up to 1 m short of H; the exponential FUN_00435370 (taken from its use);
  the vehicle and turret bodies of §10.7; whether the terrain ray's miss from below is by design; a second writer of
  BF2's E+0x340 (FUN_0023cfd0) that leaves it 0.13-0.28 m off the eye in four held states while the aim origin stays
  exact.


### 10.10 Controller follow-up, November 13, 2026

The original BF1 term in FUN_0025fea8 (181096–181110) projects relative
velocity onto the aim lateral axis and weights it by one movement axis.
Openbf reads both movement axes for ordinary tracking and matches full
shooter-minus-lead velocity while firing. Scope tracking uses the visible
body for instant shots. It remains at the configured strength through
half the weapon's nominal range, then fades to zero at that range.
Measure: controller tracking on crossing and co-moving targets, and
scoped tracking at 50, 64, 96, 120 and 128 m.

BF2 FUN_0033a658 (331689–331775) checks the human launcher's authored
target kind, sight and firing cone. Openbf keeps vehicle guidance and
gives heavy launchers half-strength passive controller assistance against
foot soldiers without passing them to the player missile lock.
Measure: matched infantry and vehicle shots with a controller, on ground
and in a space hangar. The bot guidance path has its own replay evidence
in issue #164.
