# OPUS RE (2026-09-30) - [O] UNAUDITED — Weapon slots past the primary: channels, grenades, mines, detpacks, the AI's grenade (card GREN)

Corpora: B1 = `analysis/SWBFRONT.ELF-decomp.c` (SLUS-20898), B2 = `analysis/SWBFRNT2.ELF-decomp.c` (SLUS-21240); line
numbers are of those files. Tags: [V] read at the cited lines, [M] measured on the PS2 (PCSX2, live RAM), [D] derived,
[H] hypothesis, [O] load-bearing for the port. Anything not [V]/[M] is PROVISIONAL in code.
Scratch (gitignored): `engine/artifacts/lead_tmp/gren/` — `NOTES.md`, `disc.py` / `slots.py` / `tables.py` (disc census),
`probe1.py` / `probe2.py` / `analyze1.py` (the live probes), `bf1_throw_a.json` / `bf2_throw_a.json` (their rows),
`check_*.txt` (the re_check output), `verify/` (the verifier).

## 0. What the PS2 does, in one page [O]

| question | BF1 | BF2 | tag |
|---|---|---|---|
| slots per soldier class | up to 8 (`WeaponName1..4` on the disc), each with ammo (clips) and a channel 0 / 1 | same, the class copied from its parent (unit-audit-bf2 §1.1) | V |
| what the soldier holds | every slot as a live weapon; one CURRENT slot per channel; both channels' current weapons are selected at once | same | V, M |
| the two triggers | channel 0 = primary fire (R1), channel 1 = secondary fire (L1); one channel is ACTIVE at a time, channel 1 wins, the other trigger is swallowed while a weapon fires | same | V, M |
| switching | one switch input per channel steps to the next slot of that channel (wraps); instant | same; award weapons are skipped until unlocked | V |
| ammo | `WeaponAmmoN` = clips; one clip loaded + (N-1) in reserve; a grenade is a 1-round clip; < 1 = infinite | same (an ammo object, shareable between slots) | V, M |
| grenade throw | press (fresh press only) -> 0.30 s wind-up -> release; next throw 1.70 s after the press; one grenade off the count | 0.51 s wind-up (0.533 at 30 Hz) and 1.81 s | V, M |
| launch | from the weapon's fire point, speed = ordnance `Velocity` (25), direction = normalise(aim + (0, `ShotElevate` 0.21, 0)) for a human, the aim itself for a bot | from the class's per-variant hand offset in the soldier's frame ((0, 1.818, 0.743) m for the strong throw) | V, M |
| flight | gravity `Gravity` x -9.8 (thermal: -12.25 m/s2), 30 Hz, one swept ray per frame | same | V, M |
| bounce | restitution 0.37 and friction min(1, 1.15 x 1.37 x \|vn\| / \|vt\|) — CONSTANTS in the code; the disc's `Rebound` / `Friction` on a `sticky` are never read | same constants | V, M |
| stick | per target kind from the `Stick*` mask (thermal: nothing in BF1, vehicles + buildings in BF2; crack grenade: vehicles + buildings) | same | V |
| coming to rest | after 4 bounces on a floor (normal.y > 0.7) | after 4 bounces that leave it slower than 1 m/s | V, M |
| the fuse | `LifeSpan` starts when the grenade comes to REST (or sticks, or hits water) — not at the throw: thermal 0.9 s | same: 0.6 s | V, M |
| explosion | the ordnance's `ExplosionExpire` class at the rest point (area damage: re-vehicle-death §2-§5, already in `Sim::explode`) | same | V |
| mines | dropped by a `dispenser`; land, arm at once; trigger = an enemy inside `TriggerRadius` whose velocity no longer points at the mine; a friend inside the second radius suppresses it; shot dead -> the harmless explosion; at most `MaxItems` per unit | + stick to world / vehicles, explode on enemy contact (landmine), 60 s life | V |
| time bomb / detpack | a `mine` with `LifeSpan` (10 s) and a tick sound (the Wookiee) | time bomb 5 s; detpack = `detonator` weapon: place one, the next press sets its trigger flag | V |
| the AI's grenade | in its weapon choice a grenade in [MinRange, MaxRange] (8-24 m) against a matching target kind is forced for a 3 s window every 30 s per unit, or at any time when the target stands in a GROUP (>= 3 soldiers, none of the thrower's friends within 9 m); aim = the exact low-arc ballistic solution | same window and group test (> 3 same-side soldiers, no enemy of theirs within 9 m), always against droidekas, + a per-0.1 s roll at chance t (the difficulty curve) | V |

The live numbers (§8): a BF1 thermal detonator explodes 3.64-3.70 s after it leaves the hand (flat-to-downhill Naboo
grass, 39 m), a BF2 one 2.93 s after (uphill Hoth snow, 24 m).

## 1. Method

Hops: the class-label tables (which constructor a `ClassLabel` builds) -> each class's constructor (defaults), property
setter (hash -> field) and vtable -> the instance state functions -> the soldier's constructor and update (slots,
channels) -> the AI's weapon choice -> the disc (every `wpnc` / `ordc` / `expc` / placed `entc`, both games) -> two
live PCSX2 runs. Checks run: §11. The first function block read for "the grenade" was the wrong class (`laser`,
`FUN_0028f6f0`-`FUN_0028fe38`): the RTTI init of a class sits in the middle of its code, so classes are taken from the
label table, not from the RTTI string's xref.

Class label -> class constructor [V]:
- B1 weapons `FUN_001727b8`: `grenade` (0x526fd0) -> `FUN_0028f1f0`; `dispenser` -> `FUN_0028de58`; `detonator` ->
  `FUN_0028c320`; `remote` -> `FUN_00292350`; `destruct` -> `FUN_0028bbb8`; `laser` (0x526ff0) -> `FUN_0028fd70`;
  `weapon` / `cannon` -> `FUN_002899c0`. B1 ordnance `FUN_00172568`: `grenade` -> `FUN_0027b478`; `sticky` (0x526fd8)
  -> `FUN_0027f9d8`; `shell` -> `FUN_0027e598`; `bullet` -> `FUN_00277248`.
- B2 weapons `FUN_001739d0`: `grenade` (0x60f978) -> `FUN_00337fd0`; `dispenser` -> `FUN_00336858`; `detonator` ->
  `FUN_003340c8`. B2 ordnance `FUN_00173758`: `grenade` -> `FUN_0031a458`; `sticky` (0x60f9b8) -> `FUN_003217c0`;
  `shell` -> `FUN_003203f0`.
- The disc (`disc.py bases`): BF1 16 `grenade` weapons (thermal, crack, haywire, the ALL / IMP / CIS mortar launcher, the
  Ewok rock and spear), 10 `dispenser`, 0 `detonator`; ordnance 14 `sticky`, 1 `grenade` (the Gungan haywire), mines
  4 `entc mine`. BF2 14 `grenade`, 19 `dispenser`, 5 `detonator`; 13 `sticky`, 0 `grenade` ordnance, 13 `entc mine`.

## 2. Slots and channels

### 2.1 The class [V]

- B1 soldier class setter `FUN_0020f2e0`: weapon class pointer of slot i at class+0x186c+4i (`WeaponName1` 122954 ->
  123607-123617; `WeaponName2` 122997; `WeaponName3` 122924-122934; `WeaponName4` 123025-123035), ammo at
  class+0x188c+4i (`WeaponAmmo1..3` 123439-123445, 123431-123437, 123406-123412: a value < 1 is stored as 0x7fffffff),
  channel at class+0x18ac+4i (`WeaponChannel1` 122943-122945, `WeaponChannel3` 122860-122862), slot count
  class+0x18cc = the highest slot written (123619-123623). `WEAPONSECTION n` sets the cursor `DAT_005178c4` = n-1
  (123556-123559) for the unnumbered `WeaponName` / `WeaponAmmo` / `WeaponChannel` (123333-123348, 123594-123601,
  122904-122906). The three arrays are 8 entries long (0x186c..0x188c..0x18ac..0x18cc).
- B2 soldier class: slots class+0x7f8+4i, ammo WORDS class+0x818+4i, channel BYTES class+0x838+i, count byte
  class+0x840 (unit-audit-bf2 §1.1). `WeaponAmmo` (B2 `FUN_00252bb0` 171079-171110): value < 1 sets bit 29 (infinite),
  else the low 24 bits hold the clip count. Bit 31 of the word = "share the ammo object of slot (word & 0xffffff)",
  bit 30 = share the energy object (B2 `FUN_00234a68` 149618-149632); which key sets them was not traced [H].

### 2.2 The soldier [V]

- B1 constructor `FUN_001ff1e8`: for each class slot with a class it calls the weapon class's vtable +0x10 with
  {owner controllable, aimer, the TRIGGER pointer soldier+0x2e8 + 2*channel, the reload pointer soldier+0x2ec, the
  class ammo}, into soldier+0xbb0+4i (111683-111700). Then cur[ch] (soldier+0xbd0 / +0xbd4) = the first slot of that
  channel, -1 if none (111704-111708, 111839-111842); active channel soldier+0xbd8 = 0 and active weapon index
  soldier+0xbdc = cur[0] (111715-111720).
- `FUN_0020cbd0` (121377-121402) selects the current weapon of BOTH channels (weapon+0xc4 = 1 through vtable +0x54);
  `FUN_0020cc80` deselects both. A weapon acts on its trigger only while selected (state 0, §3.2).
- B2 constructor `FUN_00234a68` (149596-149650): the same call, trigger pointer soldier+0x2a8 + 4*channel (packed
  32-bit trigger words), weapons soldier+0x7c0+4i; cur[ch] bytes soldier+0x7e0 / +0x7e1 (149652-149683).
- [M] BF1 stormtrooper (live): slots rifle (ch 0, ammo 5), pistol (ch 0, infinite), thermal detonator (ch 1, ammo 4),
  concussion grenade (ch 1, ammo 2); cur = [0, 2]. BF2 snowtrooper: rifle 5, pistol infinite (word 0x20000000),
  thermal detonator 4 (ch 1), award rifle, award pistol (ch 0; class+0x1e2 = 1 / 0, the medal type); cur = [0, 2].

### 2.3 The active channel [V, M]

B1 soldier update `FUN_00205cc8` 117099-117161, every frame:
1. If the channel-1 current weapon is in a fire or charge state (weapon+0xcc in 1..4), the active channel is 1.
2. Else the two trigger bytes are examined, channel 1 first: a pressed, held or just-released trigger (byte >= 1 or
   -1) on a weapon that is not out of ammo (state != 8) makes its channel active; once a channel is active the OTHER
   channel's trigger word is zeroed.
3. While a channel is active both switch inputs are zeroed (117145-117153). With no trigger the active channel is 0.
   While the soldier's busy timer soldier[0x303] is > 0 the triggers are not examined and a pressed one is cleared
   (117089-117100, 117129).
4. When the active weapon index changes, soldier+0xbd8 / +0xbdc are updated and the aimer is pointed at the new
   weapon (`FUN_0026e040`): the drawn weapon and the animation follow the active channel.
[M] On the L1 press the BF1 soldier's active channel went 0 -> 1 with the grenade's fire state and back to 0 at its
reload 1.73 s later; BF2's packed word (soldier+0x7e0) went 0x200 -> 0x120200 (active index 2, channel 1) and back.

### 2.4 Switching [V]

B1 `FUN_00205cc8` 117786-117838, per channel ch: when the switch input soldier[0xc4+ch] (a float, its sign the
direction) is not 0 and cur[ch] >= 0, step the index (+1 / -1, wrapping over the class slot count) until a slot whose
class channel is ch (or back at cur[ch]). Same slot again -> the error sound for a human. Else deselect the old
weapon (vtable +0x58), cur[ch] = the new slot, select it (vtable +0x54). The switch is instant — no draw time in this
path. The tooltip ids are `DAT_00518900[ch]` = 1 / 2 = `game.tooltips.controls.changeprimary` / `changesecondary`.
B2 `FUN_00240bf8` 161102-161190: the same loop over the byte channels, with one more condition on the candidate
(161138-161154; the second argument of `FUN_00360018` is dropped by the decompile, disassembly 0x245468-0x2454dc).
With x = *(soldier[0xca] + 0x1a0) (0 when soldier[0xca] is 0) and `FUN_00360018(x, n)` = bit n of the u16 at x+0x4c
(357279-357283):
- x == 0: the slot passes only when class+0x1e2 (`MedalsTypeToUnlock`, short) == -1;
- else it passes when (class+0x1e2 == -1 or bit class+0x1e2 is set) and not (class+0x1e4 (`MedalsTypeToLock`) != -1
  and bit class+0x1e4 is set).
So the award slots (UAUD2) are in every soldier and outside the cycle until the unit's medal bit is set, and an
award weapon locks the weapon it replaces out of the cycle. Who sets the bits (the award rules) is PLAN M4, not read.
Both loops: the switch runs only when no channel is active (§2.3) and the candidate is not tested for ammo.

### 2.5 Ammo [V, M]

B1 weapon constructor `FUN_002838d8` (207891-): weapon+0x90 = reserve clips = class ammo - 1 (float; FLT_MAX when the
class ammo is 0x7fffffff), +0x94 = the maximum, +0x98 = the loaded clip as a fraction (1.0), +0x9c = heat.
`FUN_00284018` = rounds in the clip = int(clip x `RoundsPerClip`), `FUN_00284060` = reserve rounds. Reload exit
`FUN_002854c0` (209084-209102) moves min(reserve, 1) clip from the reserve into the clip; reload entry `FUN_00285398`
returns the partial clip to the reserve first. A human with the infinite-ammo cheat (`DAT_00519064`) skips both.
[M] BF1 thermal: reserve 3.0 / clip 1.0 before; clip 0.0001 at the release; reserve 2.0 / clip 1.0 after the 0.01 s
reload. BF2: the ammo object (weapon+0x90) reads [.., 4.0, 4.0, reserve 3.0, clip 1.0] -> clip 0.0 -> reserve 2.0.
The BF2 HUD showed "4" beside the detonator icon = clip + reserve rounds.

## 3. The grenade weapon

### 3.1 Class keys [V]

| key (hash) | B1 class off (`FUN_0028f308` 215976-) | B2 class off (`FUN_00338190` 329961-) | constructor default (B1 `FUN_0028f1f0` 215923, B2 `FUN_00337fd0` 329894) |
|---|---|---|---|
| ShotDelay (0xd9356908) | +0x1e0, min 0.01 | +0x240, min 0.01 | 0.125 |
| InitialSalvoDelay (0xc8388d9d) | +0x1e4 | +0x248, copied to +0x244 and +0x24c | B1 0.3; B2 0.5 / 0.36 / 0.584 for variants 0 / 1 / 2 |
| InitialSalvoDelayUnderhand (0x2a708f20) / InitialSalvoDelayProne (0x36f0bfcd) | - | +0x244 / +0x24c alone; on no disc class | - |
| ShotElevate (0xae3db60d) | +0x1e8 | +0x250 | 0.3 |
| MaxPressedTime (0x2819ffa0) | +0x1ec | +0x254 | 0 |
| HideOnFire (0x5578d44c) | +0x1f0 | +0x258 (byte) | 1 |
| (0x079c1d86) fire-variant override | +0x1f4 (byte) | +0x259 (byte; >= 2 -> -1) | -1 |
| ReloadTime (base) | +0x74 | +0xac | 0.6 for this class |
| TriggerSingle (0xed9d56db, base) | +0xcc | bit 50 of +0x1e0 | 1 (B1 base ctor 209496; setter 210357). On the BF1 disc only the three mortar launchers author it ("1", the default); the thermal, crack and haywire grenades do not. A second key 0x0ee66c41 also writes +0xcc (B1 209915-209917); on no disc |

Every key else goes to the base weapon setter (B1 `FUN_002864f0`, B2 `FUN_00328fb0`).
[M] RAM, thermal detonator: B1 ShotDelay 1.4, InitialSalvoDelay 0.3, ShotElevate 0.21, MaxPressedTime 0, HideOnFire 1,
byte -1, RoundsPerClip 1, ReloadTime 0.01, class+0xcc 1. B2 1.3, [0.51, 0.51, 0.51], 0.21, 0, 1, -1, ReloadTime 0.01,
class+0x1e0 = 0x2684ffffffffffff (bit 50 set), launch offsets (0, 1.1913, 0.7068) and (0, 1.8183, 0.7433).

### 3.2 The state machine [V]

The base weapon update B1 `FUN_00284318` (208448-208690) switches on weapon+0xcc: 0 idle, 1-3 fire, 4 charge,
5 reload, 7 overheated idle, 8 out of ammo. From idle, while selected: the fire predicate (vtable +0xa8) -> the
fire state `class+0x198 + 1` and the fire-entry callback (+0x74); else the charge predicate (+0xa4) -> 4. In fire:
when the "fire finished" predicate (+0xac) holds, exit; an empty clip (+0xb8) then goes to reload (5) — or to 8 when
the reserve is empty too (+0xb0) — else back to idle. In reload: done when the timer reaches `ReloadTime`.
The grenade's slots (vtable 0x53bf48):
- fire predicate, 0x28ee20 (not in the decompile; disassembled): not overheated and — `MaxPressedTime` > 0: the
  trigger byte == -1 (released); else `TriggerSingle`: the byte == 1 (a fresh press); else: the byte > 0 (held).
  Charge predicate 0x28ede0: `MaxPressedTime` > 0 and the byte > 0.
- fire entry `FUN_0028eb30` (215619-215657): weapon+0xbc = 1 when `HideOnFire` (the weapon model is not drawn,
  `FUN_00285650` returns at once on +0xbc); strength weapon+0x144 = charge time / `MaxPressedTime`, or 1.0 when that
  is 0; the fire state = 3 for strength > 0.7, 2 for > 0.4, else unchanged (the override byte + 1 when it is >= 0);
  timer +0xd0 = 0; limit +0xd4 = `InitialSalvoDelay` + `ShotDelay`.
- fire update `FUN_0028ec38` (215661-215685): timer += dt; a deselected weapon is marked thrown; when not yet thrown
  and timer >= `InitialSalvoDelay`: if the clip holds a round (class+0x64 <= clip), throw (`FUN_0028ef70`) and take
  class+0x64 off the clip; mark thrown.
- fire finished `FUN_0028ee90` (215747-215783): thrown and timer >= limit and (`TriggerSingle`, or the fire predicate
  false, or the clip empty).
- reload exit `FUN_0028edc0`: weapon+0xbc = 0 (drawn again) then the base refill.
B2 is the same machine (state weapon+0xb8, timer +0xbc, limit +0xc0, flags u64 +0xb0: bit 34 selected, bit 32
hidden): fire update `FUN_003377d0` (329519-329554), finished `FUN_00337a98` (329641), fire predicate `FUN_00337a20`
(329614: `MaxPressedTime` <= 0 -> trigger bit 0, else bits 0-1 == 2). The fire entry is at 0x337678 (not in the
decompile; disassembled): hidden bit, strength as BF1, variant a1 = the override byte if >= 0, else 1 when strength
> 0.4 else 0; state = a1 + 1; the delay / offset index weapon+0x122 = a1, or 2 when the weapon's stance bits
(flags & 0x1f800000000) equal 0x1000000000; limit = `ShotDelay` + `InitialSalvoDelay[index]`; with `TriggerSingle`
the trigger word's state is set to 0x1c (press consumed). BF2's three indices are underhand (0, strength <= 0.4),
overhand (1) and prone (2) — the key names of §3.1; BF1 has three fire states and one delay.
The BF2 throw also needs a round in the ammo object (`FUN_00303860`) and, for a unit whose energy flag is set, the
class's energy cost <= 0 (329536-329543). In both games a deselected weapon is marked thrown before the test, so it
never throws.
A grenade is `TriggerSingle` by default: one throw per press; holding does nothing more.
[M] BF1: press -> state 3, hidden, limit 1.7 at t = 0.03 s; thrown at timer 0.3003; state 5 at timer 1.7017; state 0
0.03 s later with the reserve down one. BF2: state 2 (variant 1), limit 1.81, trigger word 0x1c; thrown at timer
0.5333 (the first 30 Hz step >= 0.51); state 4 (BF2's reload) at 1.8333; state 0 next.

### 3.3 The throw [V, M]

B1 `FUN_0028ef70` (215798-215919): position = the aimer's fire point (aimer+0x90), direction = the aimer's aim
(aimer+0x50); when the owner has a player index (owner+0xe8 >= 0) `ShotElevate` is added to direction.y and the
vector renormalised (215847-215862) — a bot's direction is used as it is (§7.3); the launch record {position,
direction, strength, owner entity, team, weapon class, lock target, weapon} goes to the ordnance class's vtable +0x10.
B2 `FUN_00337b80` (329692-329890): position = the owner's matrix x the class offset of the variant (class+0x260 +
16 x index), the offset first scaled by the soldier class's +0x1220 when that is not 1.0; the same `ShotElevate`
rule on owner+0xc0 >= 0. The offsets are filled at class finalise (`FUN_003383f8` 330057-330155) from the throw
animations sampled at the variant's `InitialSalvoDelay` — the hand's position at the release.
The launch speed is the ordnance's `Velocity`, blended through its charge knots by
the bullet constructor (B1 `FUN_002762a0`, 197533-197546). The sampled thermal
classes point to themselves in RAM; their strength changes the animation only.
That observation does not generalize to all thrown ordnance: BF1 Ewok spear
authors 5 -> 25 m/s and rock 5 -> 20 m/s at `NextCharge 1.0`; Gungan haywire
authors 1.5 -> 25 m/s. Their weapons have no press limit. Fire entry
`FUN_0028eb30` B1:215634-215640 supplies strength 1 in that case, which the
throw packet carries (`FUN_0028ef70`, 215865). Sticky inherits the shell,
bullet and base constructors' charge evaluation; `re-weapon-charge` §4 gives
the complete BF1/BF2 chain. These native code and authored-data findings do
not constitute new retail Ewok or mortar throw captures.
[M] BF1: aim (-0.1205, -0.0622, 0.9908) -> launch velocity (-2.9858, 3.6617, 24.5495) = normalise(aim + 0.21 y) x 25
to 4 decimals; BF2: aim (0.0064, 0.1857, -0.9826) -> (0.1513, 9.3390, -23.1896), the same formula; BF2 spawn position
= feet + (0, 1.8183, 0) + 0.7433 m along the facing.

The four finalized BF2 stock thermal classes share the strong local hand point
`(0, 1.8182718, 0.74327886)` in retained retail states. The soldier property
setter stores `SkeletonRootScale` independently of collision scale (B2:172070–172080).
The thermal launch reads that value and the owner world matrix through separate
calls (`FUN_00337b80`, B2:329765–329795). The soldier owner slot resolves to
`0x255890`, which returns the matrix at entity +0x120. Retained Kamino poses
keep this basis upright through slope contact and falls. The original samples
do not include a live scaled, prone or posture-transition thermal release.

The port scales the strong hand vector for the four native uncharged BF2
thermal classes on standing and crouched releases. The 2 cm world-Y floor
correction comes after the scale. Unity and compatibility throws keep their
calibrated bits. OURS: other release stances and authored variant overrides
keep their existing origin. trace: a live scaled release and the latched
variant across posture changes, including the port's restored prone.

## 4. The thrown ordnance (`sticky`; the `grenade` BASE in §4.6)

### 4.1 Flight [V, M]

- The base ordnance (B1 constructor `FUN_002728a8` 195037-, update `FUN_00272e98` 195346-195375): life s+0x3c =
  `LifeSpan`; the life timer runs only while the byte s+0x39 is set; at life <= 0 the class's `ExplosionExpire`
  (class+0x34) is spawned at the position and the ordnance dies. It also dies below y = -500 (B2 -2000).
- The shell (B1 class `FUN_0027e598` 203544, setter `FUN_0027e668` 203585, instance `FUN_0027e398`, update
  `FUN_0027e478` 203497-203542): gravity field s+0x110 = `Gravity` x -9.8 (default 1.0); each update, above water:
  velocity.y += gravity x dt, then the bullet update (`FUN_002764c0` 197603-): position += velocity x min(life, dt).
  Under water: velocity -= (0.7 dt / (0.7 dt + 1)) x (vx, vy + 0.5, vz). Defaults `LifeSpan` 3.0, `Velocity` 35.0.
  B2 identical (`FUN_003203f0` 313344, `FUN_003204c0` 313385, `FUN_003202d0` 313297, base `FUN_0030f988` 301804).
- The sticky instance (B1 `FUN_0027eb18` 203813-203832; B2 `FUN_003207e8` 313513): s+0x39 = NOT class flag
  (B1 class+0xf8, B2 class+0x138; key 0x0d0cde2b; constructor default 1) — so the life timer is NOT running at the
  spawn. The key is on no class of either disc (census over 1108 + 2205 classes) [V] and RAM shows 1 [M].
[M] gravity field -12.25 on both; update step 0.03337 s (BF1) / 0.03333 s (BF2) = one per frame at 30 Hz; free
flight rows change only by gravity x dt.

### 4.2 The sweep and the contact [V]

The collision manager calls every moving ordnance's vtable +0x38 once a frame (B1 `FUN_00163198` 2720-2745). The
sticky's is `FUN_0027eff8` (204096-204185): a ray from (previous position - 0.1 x direction) over (step length +
0.2), layer mask 0x9de, ignoring the owner; on a hit, vtable +0x3c = `FUN_0027f1e8` (204187-204607) with the body,
the point and the normal. B2: `FUN_00320de8` (313836, mask 0x13ba) -> `FUN_00320fe8` (313931-314350).
The handler, in order:
1. The first contact stores the time (B1 s+0x12c, B2 s+0x16c) — it starts the AI alert (§7.4).
2. Water (B1 layer 0x80, B2 0x100): velocity = (0, -0.2, 0) (B2 (0, -2.0, 0)), the water sound, ARM the timer, return.
   Foliage (B1 0x100, B2 0x200): its sound, nothing else.
3. Any other contact plays `CollisionOtherSound`. Stick? Terrain (B1 0x40, B2 0x80): bit 31 of the class mask.
   An entity: bit (entity kind & 7) of the mask — and the entity takes the ordnance's direct damage through its
   damage interface (the same descriptor a bolt uses, `FUN_00272f88`).
4. Stick (and the record pool has room): a record {entity, id, body, bone, local position, local orientation}; the
   grenade rides it (update B1 `FUN_0027ec30` 203858-203955); position = point + 0.01 x normal; velocity 0; it leaves
   the sweep list; ARM the timer.
5. Else bounce, while the bounce count is <= 3 (§4.3). At count 4, or when the velocity does not point into the
   surface: leave the sweep list, gravity 0, velocity 0, ARM the timer — the grenade is at rest.
The class mask (B1 class+0x100, B2 class+0x13c; default 0xffffffff): bit 0 `StickPerson`, 1 `StickAnimal`,
2 `StickDroid`, 3 `StickVehicle`, 4 `StickBuilding`, 5 `StickBuildingDead`, 6 `StickBuildingUnbuilt`, 31
`StickTerrain`; key 0xd3913cc2 sets or clears all (setters B1 `FUN_0027fb78` 204689-204835, B2 `FUN_00321960`
314433-). [M] RAM masks: BF1 thermal 0x7fffff80, BF2 thermal 0x7fffffd8 (vehicles, buildings, unbuilt buildings).

### 4.3 The bounce [V, M]

B1 204520-204595 (constants read from the instructions at 0x27f7f0-0x27f8c8), B2 314270-314345:
```
vn = dot(v, n)                      # n = the contact normal
if vn >= 0: count = 4               # -> rest
else:
    position = point + 0.1 * n
    vt = v - vn * n
    f  = min(1, (-vn * 1.15 * 1.37) / |vt|)
    v  = vt * (1 - f) - n * (vn * 0.37)
    BF1: if n.y > 0.7: count += 1   # a floor
    BF2: if dot(v, v) < 1.0: count += 1   # a slow bounce
if count >= 4: rest
```
The sticky class has no `Rebound` / `Friction` field: both hashes are read only by the `grenade` ordnance class and by
the item entity (B1 `FUN_0027b640`, `FUN_001f4608`; B2 `FUN_0031a5f0`, `FUN_002263c0`, `FUN_0022dc20`) [V, absent].
The disc's `Rebound 0.3 / Friction 1.15` (BF1) and `0.2 / 2.0` (BF2) on the thermal detonator change nothing.
The velocity is changed in the sweep, AFTER that frame's update; the next update adds gravity to the new velocity.
[M] Every sampled bounce fits this with one terrain normal per site: BF1 residual <= 0.007 m/s over 3 bounces x
2 throws with a 1-degree normal search (normal (-0.134, 0.990, -0.045)); BF2 0.0000 over 5 bounces x 2 throws (normal
(0.054, 0.944, 0.326)); the verifier's exact solve leaves <= 2.5e-6 m/s (§11).
BF1 counted 1, 2, 3, 4 on its four floor contacts. BF2 did not count its first two contacts (speed^2 after = 37.3
and 1.69) and counted the next four (0.435, 0.220, 0.158, rest).

### 4.4 The fuse [V, M]

The byte s+0x39 is set by: the base constructor (cleared again by the sticky constructor), water, a stick, and rest —
not by a bounce (B1 204595-204605, the three stores at 0x27f2b4 / 0x27f7a4 / 0x27f94c; B2 314340-314350, plus the
under-water branch of BF2's update, 313607-313609). A stick with the record pool full falls through to a bounce.
So `LifeSpan` runs from the moment the grenade stops moving.
[M] BF1: armed at the 4th floor contact (age 2.77 / 2.84 s) with life 0.9; gone 0.87-0.90 s later. BF2: armed at age
2.37 s with life 0.6; gone at 2.93 s. Before the rest the life field stayed at its start value in every row.

### 4.5 What the explosion is

`ExplosionExpire` (0x2e51081f) names an `expc` class; B1 `FUN_002701a0` / B2 `FUN_0030c8f8` spawn it with the owner
record. Its area damage, push and the per-kind scales are re-vehicle-death §2-§5 (landed as `Sim::explode`). The
disc numbers are in §9.

### 4.6 The `grenade` ordnance BASE [V]

B1 `FUN_0027b478` (201380) / setter `FUN_0027b640` (201444): `Rebound` -> class+0xe0 (default 0.5), `Friction` ->
class+0xe4 (default 0.2), used in the same bounce formula in place of 0.37 / 1.15 (handler `FUN_0027b068`
201175-201355: f = min(1, -vn x Friction x (Rebound + 1) / |vt|), normal part x Rebound); no sticking; the same
count-to-4 on floors and the same arm-at-rest. One class uses it: BF1's `gun_weap_inf_haywiredetonator_ord` (the
Gungan locals). BF2 has the class (`FUN_0031a458`) and no disc user.

## 5. Dispenser, mine, time bomb, detpack

### 5.1 The dispenser weapon [V]

B1 class `FUN_0028de58` (215037), setter `FUN_0028dfa0` (215096): the placed ENTITY class = `OrdnanceName` /
`Ordnance` (0xf8899c5e / 0x399bf05d) -> class+0x1e0; `Velocity` -> +0x1e4 (default 10.0); `ShotDelay` +0x1e8 (0.125);
`InitialSalvoDelay` +0x1ec (0.3); `MaxPressedTime` +0x1f0 (0); `MinStrength` +0x1f4 (0.3); `MaxStrength` +0x1f8 (1.0);
`MaxItems` +0x1fc (0); `HideOnFire` +0x200 (1); `ReloadTime` default 0.6.
The cycle is the grenade's (fire entry `FUN_0028d5d8` 214555, update `FUN_0028d668` 214581: dispense once at
`InitialSalvoDelay`). Dispense `FUN_0028d958` (214731-215025):
- `MaxItems` > 0 and the unit's list is full -> the OLDEST item is destroyed (`FUN_001bbc98` on its damage
  interface) and dropped from the list (214823-214855). The list and its count are per unit: the class holds an array
  of lists indexed by the owner's character slot.
- The entity is created at the fire point, oriented by the aim; its velocity = aim x (MinStrength + (MaxStrength -
  MinStrength) x strength) x Velocity + the owner's velocity (214939-214957); it takes the owner's team; a mine also
  stores its owner. With `MaxItems` the new item is appended.
The BF1 mine dispenser: MinStrength 0.3, MaxStrength 0.4, Velocity not authored (10) -> 4 m/s forward, `MaxItems` 4.

BF2 pack release differs from that BF1 fire-point path: class finalizer `FUN_00336e78` caches the offhand
animation's left-hand translation at the authored release time, and `FUN_00335920` transforms the cached
offset into the owner's frame. See `re-grenade-throw-anim.md` section 11.1 for the executable references,
baked offset measurement, port approximation and release-pose regression [O] UNAUDITED.

### 5.2 The item base and the mine [V]

- The item (B1 class `FUN_001f4408` 104222: `LifeSpan` FLT_MAX, `Rebound` 0.3, `Friction` 0.5; update
  `FUN_001f3aa0` 103771): life -= dt; not landed: velocity.y += `Gravity` x dt x -4.9 before and after the move (=
  -9.8 x Gravity per second). Contact `FUN_001f40a0` (104017-104215): soldiers (layer 2) and vehicles (4) are ignored;
  else with vn < 0: a slow contact (vn > -1.0 m/s) LANDS it — aligned to the surface normal, velocity 0; a faster one
  bounces with the class `Rebound` / `Friction` in the §4.3 formula.
- The mine (B1 class `FUN_001f9478` 107558, setter `FUN_001f96b0` 107653, update `FUN_001f8410` 106776-107146):
  trigger explosion class+0x14fc (`ExplosionName` 0x9b30e763, 0x02bb1fe0 or `ExplosionTrigger` 0x6a5e371c), expire
  explosion +0x1500 (`ExplosionExpire`), destroyed explosion +0x1504 (0xa2723442), `TriggerRadius` +0x150c
  (0xc461d811), a second radius +0x1510 (0xce754894), `TickSound` +0x14f0 with `TickSoundPitch` +0x14f8.
  Each update, in this order: life <= dt -> the expire explosion; no longer alive (shot dead) -> the destroyed
  explosion; else, once LANDED and with a trigger explosion and `TriggerRadius` > 0: gather the bodies within
  max(TriggerRadius, second radius) on layers 0x16 (soldiers, vehicles, riders); for an ENEMY whose bound sphere
  centre is within TriggerRadius + its radius: if dot(its velocity, centre - mine) >= 0 the mine is triggered; for a
  FRIEND within the second radius + its radius the trigger is cleared and the scan stops. Triggered -> the trigger
  explosion at the mine, owner credited. So a mine fires at the enemy's closest approach (or at once under a standing
  enemy), and never with a friend inside the second radius. The discs author that radius on one class only,
  `com_weap_inf_empmine` (2.5, with TriggerRadius 3.0), which no disc weapon dispenses; on the landmine it is 0.
  `FUN_001f8400` sets the triggered flag directly (mine+0x314).
- B2 (`FUN_0022c558` 143645, `FUN_0022c7a8` 143740, update `FUN_0022b2f8` 142813-143223, contact `FUN_0022bc58`
  143225-143643): the same update with layers 0x2a and a soldier-class flag (class+0x8d8) that exempts a unit. The
  contact differs: for its first 0.5 s the mine passes through its owner; it STICKS to the world and to entities of
  kind > 2 (vehicles, buildings) and bounces off people, animals and droids; once landed, with key 0xbfb22e35 set
  (the landmine), an enemy touching it sets it off at once. The `Stick*` keys on BF2's detpack and mine entities are
  never read (only the sticky ordnance setter holds those hashes) [V, absent].

### 5.3 Time bomb and detpack [V]

- The time bomb is a mine with `LifeSpan` and `ExplosionExpire` = the damaging explosion, no `TriggerRadius`: BF1's
  Wookiee `remote_charge_dispenser` (10 s, `MaxItems` 3), BF2's `timebomb` (5 s). The tick sound plays each whole
  second, its pitch rising toward the end.
- BF2's detpack weapon is the `detonator` class, a dispenser subclass (`FUN_003340c8`): while the unit has a live
  item (`FUN_00333528` 326607) its fire and charge predicates are false (`FUN_00333608` / `FUN_003335c8`); the update
  `FUN_003336a0` (326697-326792), on a FRESH press with items out, calls each item's damage-interface vtable +0xc =
  `FUN_0022ca80` -> `FUN_0022b170` = "set the triggered flag" (mine+0x2e4), empties the list and consumes the press.
  The pack then fires its trigger explosion on its next update. The pack itself: `MaxItems` 1, `MaxPressedTime` 0.5
  (a charged throw: 2 to 10 m/s), sticks as §5.2, 60 s life, 20 health (a shot destroys it harmlessly).
- BF1 has the `detonator` class (`FUN_0028c320`) and no disc weapon of it.

## 6. The human's side [V, M]

The semantic the PS2 exposes is: fire primary, fire secondary, switch primary, switch secondary, reload (+ the
charge by holding, for weapons with `MaxPressedTime`). L1 threw the grenade in both live runs. The pad binding of the
two switch inputs was not traced (the port's controls are its own; FEEL_NOTES).
A grenade press while the primary fires: channel 1 wins (§2.3) — the primary's trigger is swallowed until the
grenade's fire state ends (1.7 / 1.81 s), and the soldier shows the grenade, not the gun, for that time.

## 7. The AI

### 7.1 The weapon choice [V]

B1 `FUN_002387b0` (151834-152003), B2 `FUN_002ab590` (230907-231130) — the "situation query" of re-ai-difficulty §5,
called from the attack-suitability handlers for the unit's target. For every slot, in slot order:
1. skip a weapon with no ammo at all (B1 `FUN_002840c0` <= 0; B2 `FUN_003037e8`);
2. skip one the suitability test rejects (B1 `FUN_002380a8`, B2 `FUN_002aaba0`: the weapon's `Target*` mask against
   the target's kind, then an expected-damage score);
3. "plenty" = more than 20 rounds left (B1) / more than 2 (B2);
4. IN RANGE (`MinRange`^2 < d^2 < range^2): the first in-range weapon is taken; a later one replaces it when it has
   plenty and the best has not, or when both agree on plenty and its score is higher. Then the grenade test (§7.2)
   may force it. Taking a weapon sets "in range" for the caller;
5. out of range, and nothing in range yet: a fallback by the same preference inside the weapon's extreme range.
When the caller asks for it and the winner is not already the current weapon (151910-151924), its slot is requested
with B1 `FUN_0023d3f0` (the unit's Controllable +0x17c = soldier+0x41c) / B2 `FUN_002bbed0`; the Controllable's next
step (B1 `FUN_0023d418` 155532-155645) deselects the channel's current weapon, makes the slot current and selects it
— at once — and records the slot's channel as the AI's firing channel (+0x180). A later in-range weapon in the same
pass can still replace a forced grenade by the rule of step 4.

### 7.2 The grenade test [V]

- B1 `FUN_00238678` (151777-151831; `DAT_00589498` = the id of `WeaponGrenadeClass`, 216053-216062): for a soldier
  attacker, the weapon is forced when its class is a grenade weapon and EITHER the target's GROUP stamp is fresh OR
  (unit index mod 10) == (floor(game time / 3) mod 10). No skill term in BF1.
  The group stamp: `FUN_0025d0b8` (178852-178859) = now < (the target's controller)+8 + 5.0; the stamp is written by
  `FUN_00263290` (183296-183335) when an AI takes a soldier as its target, at most once per 5 s per target: up to 10
  objects within 9.0 m of the target are gathered; if none is a friend of the AI and the soldiers among them,
  the target included, number more than 2, controller+8 = now.
- B2 `FUN_002ab288` (230788-230840): the weapon class must be `DAT_00699f40` (WeaponGrenadeClass); the considering
  gate is the same window, OR a droideka target (`DAT_00689300`), OR the group test `FUN_002aec90` (233525-233535) on
  the target's controller: +0x8c > 3 and +0x88 == 0, the counters rebuilt at most once a second by `FUN_002af480`
  (233894-): soldiers within 9 m of that unit, +0x88 those hostile to it, +0x8c the rest. Inside the gate the roll
  (unit index + floor(time x 10)) mod 100 <= floor(100 x t), t = `FUN_002a2d68` = the difficulty curve lerp(0, 1)
  (re-ai-difficulty §4).
So a bot considers its grenade for 3 s in every 30 s, and at once against a cluster of enemies with none of its own
side among them; with the target a matching kind (thermal: person, animal, droid) and 8-24 m away it switches to it.
The disc's grenade weapons are `TriggerSingle`, so the press itself is the single-shot tap of card STTAP (B2
`FUN_002baf70`; the fire controller `FUN_002bb1b0` drives the trigger word of the AI's firing channel and releases
the other).

### 7.3 The aim [V]

A bot's throw direction is its aimer's direction (no `ShotElevate`, §3.3). The aimer leads with the ballistic solver
B1 `FUN_0026d3f0` (191620-191715): with speed s (weapon vtable +0x2c) and gravity g < 0 (+0x30), d = target - origin:
A = s^2 + g d.y; disc = A^2 - g^2 |d|^2; if disc > 0: A -= sqrt(disc); t^2 = 2A / g^2; the aim point is raised by
-g t^2 / 2 and moved by target velocity x t. That is the exact low-arc solution; with no solution (disc <= 0) the
un-reduced A is used and the function reports failure.

### 7.4 The grenade warning [V]

After a sticky's (or grenade's) first contact, its update calls vtable +0x40 (B1 `FUN_0027f988` -> `FUN_00236be8`
150868-150940; B2 `FUN_00321770` -> `FUN_002a89b8`): once the reaction delay since the first contact has passed (BF1:
U(1.1, 2.1) s when the enemy-team difficulty `DAT_005875f4` < 14, U(0.6, 1.3) when < 17, else U(0, 0.8); BF2
re-ai-difficulty §4) and at most once a second, every AI-driven unit within 7.5 m (layers 0x1e, the thrower excluded)
gets the grenade's position through its controller's vtable +0x30. What the controller does with it was not traced
[H].

### 7.5 Audit of 7.1-7.3 against both ELFs and the PS2 (card GRAI's audit, 2026-10-07: PARTLY)

Read first-hand by the audit of landing 79f1890, with `re_check.py fn / word` and the writer searches in
`engine/artifacts/lead_tmp/audit-grai/verify/re_checks.txt`. **No verifier pass** (the session's agents went to the
landing's numbers, mutations and controls): card GRAIA runs one before it builds on the corrections.
**Held** [V]: the slot walk, the range band and the preference of 7.1 step 4 in both games; the grenade gate, the
window, BF2's roll; the bot's number is `FUN_002a3f08(0, 100)`, drawn once in the controller's constructor
(B2:238826); the solver of 7.3. BF2's cluster counters `FUN_002af480` (B2:233894-233990) skip the controller itself
(233926), count soldiers within 9 m in 3-D (81.0 at 233942 / 233973), +0x88 those the relation table marks hostile
and +0x8c the rest - "+0x8c > 3" is more than 3 OTHERS, 5 soldiers with the unit.
**Corrections** [V]:
1. **"Plenty" is the load the weapon was ISSUED, not the rounds left** (step 3 says "left"). B1 `FUN_00238090`
   (151561-151567) = weapon+0x94 x class+0x60 (rounds per clip) > 20 (151950); weapon+0x94 has one writer in the
   weapon code, the constructor `FUN_002838d8` (207922: the clip count it is given; +0x90 = that - 1, +0x98 = 1.0),
   while "has ammo" `FUN_002840c0` (208239-208249) reads +0x98 + +0x90. B2 `FUN_002a9ef0` -> `FUN_00303788`
   (294536-294546) = ammo[2] x clip size > 2 (231058); ammo[2] is written once, by the constructor `FUN_00303670`
   (294472: [1] = [2] = [3] = the count), and is the cap `FUN_003038c8` (294585-294600) clamps the reserve [3] to;
   "has ammo" `FUN_003037e8` reads [3] + [4]. So a rifle is "plenty" until it is empty, a BF1 thermal detonator
   (3-4 issued) never is, a BF2 one is with 4 issued and is not with 2.
2. **BF1's score is one shot's damage, not a rate.** `FUN_002380a8` (151570-151776): ordnance damage (ord+0x4c) x the
   scale of the target's kind (151666) plus each explosion's (`FUN_00237ff8`, 151545-151559); "too weak" = min(the
   issued rounds, 50) x that <= half the target's max health (151679-151681). Its mask word is class+0xf8 (the word
   the `Target*` keys write, 209954-210551), tested only when attacker AND target are soldiers (151647-151654). BF2's rate
   (`FUN_002aa548` / `FUN_002aa630`) is BF2's alone.
3. **BF2 does not choose a weapon that is reloading.** When the caller applies the choice (argument 3), a weapon whose
   state (+0xb8) is 4 or 5 is skipped unless its vtable +0x5c returns nonzero (231043-231050). +0x5c is `jr ra; move
   v0, zero` (0x32b598) in 15 of the 16 weapon vtables and returns 1 in one (0x629838) [ELF words]; state 4 is the
   reload (`FUN_00326170` case 4, 318261; [M] in 3.2). BF1's function has no such test.
4. BF2's fallback (step 5) skips a weapon whose vtable +0x60 returns nonzero (231099: the base returns 0, one vtable,
   0x629b00, returns 1 - the kind `FUN_002a9b78` maps to code 4, the melee mode 7) and takes any reach when argument 7
   is set; BF1's has neither.
**The verifier pass (card GRAIA, 2026-10-09; one opus verifier given the claims and citations only): 8 claims, 7
SUPPORTED, 1 PARTLY, 0 not.** Supported as written: corrections 2, 3 and 4, BF1's "plenty" (B1:151566 the product,
151950 *(decompiler detail omitted)*, strict; +0x94 written only at 207922 - the reload's start and finish, `FUN_00285398` /
`FUN_002854c0`, write +0x90 and +0x98 alone; `FUN_00256c78` 174159 uses +0x94 as the reserve's cap), BF2's rate
(`FUN_002aaba0` -> `FUN_002aa948` -> `FUN_002aa728` -> `FUN_002aa548` / `FUN_002aa630`), the +0x5c / +0x60 words (20
weapon vtables enumerated twice, by the shared slot +0x28 and by a 4-of-28 slot match: 19 hold 0x32b598 at +0x5c and
one, 0x629838, holds 0x344f70; 19 hold 0x32b5a0 at +0x60 and one, 0x629b00, holds 0x345ee0 - correction 3's "15 of
the 16" was a short count). What it added or corrected ([V] by the verifier, not re-read by this session):
- Correction 1, BF2, PARTLY: ammo[2] has a SECOND writer. `FUN_00325840` (slot +0x28 of every weapon vtable;
  B2:317676) sets [2] = [1] x 2 and [3] = [2] when the owner's team holds the bonus whose hash 0xc81956e6 maps to
  the string "supplies" (B2:343113; `FUN_0034b1b0`) - Galactic Conquest's bonus. [1] stays the issued count. The
  port has no such bonus: its "plenty" is the issued load. The ammo object is pooled and refcounted, and the weapon
  constructor `FUN_00325448` (317537-317544) can share one between weapons; the reserve is clamped to [2] - 1 while
  the clip is not empty (294594).
- BF2's choice has one more skip when it is applied: `FUN_002ab430(...) == 0` passes the weapon over, before the
  state test. Unread [H]; BF1 has no counterpart.
- BF2's score has five outcomes by ordnance class (`FUN_002aa728`: three rates - the third, `FUN_002aa6b8`, is damage
  / (2 x `FUN_002aa0b8`) - and the constants 100 and 1000, or 0), and `FUN_002aa948` averages two calls when two
  difficulty-ranked list walks differ; its "too weak" test is rate x 10 <= half the target's health (230662), with
  no ammunition term. BF1's score returns a fixed 1000 for some class pairs (151632) before the damage, and weighs
  each explosion by `FUN_00221530`'s falloff at 2.0 m less half the target's size (151553).
- Not checked by it: the 66 integer stores to +0x94 outside 0x230000-0x2a0000 in BF1's text (a weapon written from
  another module or copied whole is not excluded - correction 1's "one writer" is [V] for the weapon code, [H] beyond
  it); stores to ammo[2] through a pointer passed outside 0x2a0000-0x380000; state 5; the class of vtable 0x629838.

**Built (card GRAIA):** corrections 1-3 (`arsenal::choose`, `arsenal::shot`, `WeaponState::issued`); the explosion's
falloff in BF1's score is taken as 1 and state 5 is left out, both PROVISIONAL. OURS, a named deviation: no grenade
is chosen for a target with a friend on foot within 12 m of it, or thrown where one stands or walks to within the
flight and a second (`arsenal::friend_by`) - the PS2 has no such test (its BF2 Kamino bots: 12.1 % of kills are of
their own side, card FFLN). [M], 24 seeds x 32 maps: team + self kills 1.96 -> 1.19 % of kills, no map over 3 % (BF2
Tantive IV 6.7 -> 2.9), for 22 % of the throws; in the first 210 s on Polis Massa a throw per 384 bot-seconds
becomes one per 710 (the PS2: 347-363), on Kamino 806 -> 1,052 (the PS2: 146-239).

**OURS, a named deviation (2026-11-10): a shell launcher is "plenty" on every map.** BF1's rule (issued over 20)
leaves the BF1 Imperial and CIS pilots' mortar, 3 clips of 5 = 15 rounds issued, to their endless pistol wherever both
are in range (10-32 m), so the mortar fired only at 32-48 m, about 2 rounds a life, and the Imperial pilot was the
largest class gap between the BF1 factions (`analysis/faction-balance-2026-11.md` section 2). `arsenal::choose` counts
a weapon whose ordnance is a shell as plenty. On BF2 maps every mortar already passes BF2's threshold (15-25 issued,
over 2), so only BF1 maps move. A lobbed launcher (the BF1 smuggler's) is untouched. Asset-free test:
`a_bf1_pilot_keeps_its_mortar_over_the_pistol_in_both_bands`. measure: the proxy gate and both bot-only sets of
`analysis/faction-balance-2026-11.md`, BF1 GCW and BF1 CW.

[M] **The PS2's rate** (SAIR's four BF2 captures re-read, `lead_tmp/audit-grai/ps2/`; GCW conquest, 16 a side): the
mover's request step (B2:242749-242778) sets controllable+0x84 = the requested weapon's channel and it stays until
the next request. Channel 0 -> 1 episodes: Kamino 37 in 5,391 bot-seconds and 30 in 7,180 (one per 146 / 239),
Polis Massa 15 in 5,204 and 14 in 5,087 (one per 347 / 363); median episode 2.5-5.0 s (the 3 s window). An enemy
soldier 8-24 m away: 14.4 / 14.7 % of live AI soldier samples on Kamino, 24.3 / 22.4 % on Polis Massa. Slot 1 in the
hands (E+0x7e0): 14.0 % (Kamino), 11.7 % (Polis Massa). The port on main (8 seeds, the first 210 s): one throw per
1,074 bot-seconds on Kamino with an enemy in the band 4.6 % of the time, one per 406 on Polis Massa with 22.8 %;
slot 1 in the hands 10.1 / 7.4 %.

## 8. PCSX2 live (2026-09-30, software renderer, RAM read through /dev/shm, nothing written)

| | BF1 NAB1, stormtrooper, thermal detonator | BF2 HOT1, snowtrooper, thermal detonator |
|---|---|---|
| state | `ref/states/bf1_nab1_walk_cp1_cp2_b000.p2s` -> spawn | `ref/states/bf2_hot1_walk_cp1_cp6_b000.p2s` |
| press -> fire state | 0.03 s (state 3) | 0.04-0.07 s (state 2) |
| release (timer) | 0.3003 | 0.5333 |
| fire state ends (timer) | 1.7017 | 1.8333 |
| launch speed | 25.0000 | 25.0000 |
| gravity field | -12.25 | -12.25 |
| contacts (age, s) | 1.50, 2.30 / 2.27, 2.64 / 2.57, 2.84 / 2.77 | 1.03, 1.67, 1.93, 2.10, 2.23, 2.37 |
| rest and armed (age) | 2.84 / 2.77 | 2.37 |
| gone (age) | 3.70 / 3.64 | 2.93 |
| travel | 38.7 / 39.4 m, 8 m downhill | 23.5 / 24.3 m, 3.6 m uphill |
Two throws each; the BF2 pair was identical row for row. Rows: `lead_tmp/gren/bf{1,2}_throw_a.json`; the fit:
`analyze1.py`. A replay fixture for the port (BF1, throw 1): launch position (151.703, -108.608, 18.289), velocity
(-2.9858, 3.6617, 24.5495); first contact with v = (-2.986, -14.323, 24.549) on normal (-0.134, 0.991, 0.022) ->
(-1.493, 4.644, 4.484) before the next gravity step.

## 9. The disc numbers (`tables.py`; the last PROP of a key wins, the parent's first)

BF1 weapons — thermal detonator: ShotDelay 1.4 (the Rebel one 1.3), InitialSalvoDelay 0.3, ShotElevate 0.21,
RoundsPerClip 1, ReloadTime 0.0 (the Rebel one 1.0), ranges 8 / 16 / 24, targets person + animal + droid. Crack
(concussion) grenade: the same cycle, targets vehicle + building. Haywire (EMP): no `ShotElevate` key (default 0.3),
targets droid + vehicle. Mine dispenser: ShotDelay 1.5 (ALL) / 0.0 (IMP, CIS, REP), ReloadTime 1.0 / 0.75, MaxItems 4,
strength 0.3-0.4. Remote charge: ShotDelay 0.5, ReloadTime 3.0, MaxItems 3.
Ammo per class (`slots_b1.txt`, 64 classes): thermal 3 (ALL, REP, CIS), 3 or 4 (IMP), 6 (one REP class); crack 2-3;
mines 4; remote charge 3.

| BF1 ordnance | LifeSpan | Velocity | Gravity | sticks to | explosion: Damage, radius inner / outer, Push, scales person / animal / droid / vehicle / building |
|---|---|---|---|---|---|
| thermal detonator (`sticky`) | 0.9 | 25 | 1.25 | nothing | 320, 4.5 / 6.5, 10, 1 / 1 / 1 / 0.25 / 0.25 |
| crack grenade (`sticky`) | 1.0 | 25 | 1.25 | vehicle, building, dead building | 400, 2.0 / 5.0, 0, 0.25 / 0.25 / 0.25 / 1 / 0.5 |
| haywire detonator (`sticky`) | 1.0 | 25 | 1.25 | nothing | 450, 4.5 / 6.5, -, 0.5 / 0.5 / 1 / 0 / 0.1 |
| landmine (`mine`, 200 health, TriggerRadius 1.0, no LifeSpan) | - | 0 | 1.0 | - | trigger 1750, 4.5 / 6.5, 10, 1 / 1 / 1 / 1 / 0.1; destroyed: a class with no damage keys |
| remote charge (`mine`, 100 health, LifeSpan 10) | 10 | 0 | 1.0 | - | 2000, 2.5 / 3.5, 10, 1 / 1 / 1 / 1 / 0.5 |

BF2 weapons — thermal detonator: ShotDelay 1.3, InitialSalvoDelay 0.51, ShotElevate 0.21, ranges 8 / 16 / 24, targets
person + animal + droid; ammo 2 to 4 per class (`slots_b2.txt`, 164 classes). Mine dispenser: InitialSalvoDelay 0.5,
ReloadTime 1.25, MaxItems 4, ammo 4. Time bomb: ShotDelay 0.5, ReloadTime 3.0, MaxItems 3, ammo 3. Detpack: ShotDelay
0.5, InitialSalvoDelay 0.25, ReloadTime 3.0, MaxItems 1, MaxPressedTime 0.5, strength 0.2-1.0, ammo 3.

| BF2 ordnance | LifeSpan | sticks to | explosion: Damage, radius inner / outer, Push (radius), scales person / animal / droid / vehicle / building |
|---|---|---|---|
| thermal detonator (`sticky`, Velocity 25, Gravity 1.25) | 0.6 | vehicle, building | 450, 4.5 / 6.5, 10 (6.5 / 6.5), all 1.0 |
| landmine (200 health, TriggerRadius 1.0, contact trigger) | 60 | world, vehicles, buildings (code) | trigger 8000, 4.5 / 6.5, 10 (4.5 / 4.5), 1 / 1 / 1 / 1.5 / 0.1; expire and destroyed: no damage |
| time bomb (800 health) | 5 | as the mine | 5000, 3.0 / 6.0, 16 (4 / 6), 1 / 1 / 1 / 1 / 0.5 |
| detpack (20 health) | 60 | as the mine | trigger 3000, 0.5 / 3.5, 12 (2 / 3.5), 0.4 / 0.4 / 0.4 / 1 / 0.75; expire and destroyed: 0 |

## 10. The port today, and the gaps

- `game/src/roster.rs` takes the first `WeaponName1` / `WeaponName` and its ammo; nothing reads the other slots or
  the channel (unit-audit-bf1 §3.1). `core/src/weapon.rs` models the cannon cycle only; `OrdnanceClass` has no
  gravity, bounce or stick; `core/src/skill.rs`'s `grenade_chance` has no consumer. `core/src/sim/blast.rs`
  (`Sim::explode`) already does the explosion.
- Cards: GREN (build: slots + channels + the human's throw + the sticky ordnance + the HUD count), GRAI (the AI's
  weapon choice with the grenade test and the ballistic aim), MINE (dispenser, mine, time bomb, detpack). The recon
  droid (`remote` + `destruct`), the auto turret and the powerup dispenser are not covered here (PWUP has the item).
- (2026-10-26) A thrown grenade's glow in flight is its ordnance's `TrailEffect` (BF1 thermal: `com_sfx_weap_grenade_red`),
  bound to the round at launch; a blast at or below the water + 0.3 m spawns the explosion's `WaterEffect` and no
  `Effect`. BF1 stores `LightColor` / `LightRadius` but never reads them. Measured live and ported render-side:
  re-fx.md §14.

## 11. Checks run (output in `lead_tmp/gren/check_*.txt`)

1. `re_check.py fn b1 …` over 56 cited lines and `fn b2 …` over 37: every line is in the function named above
   (`check_fn_b1.txt`, `check_fn_b2.txt`); the state machine at 208440 / 208500 is `FUN_00284318`.
2. `re_check.py absent b1 678de338 a51be2bb d0cde2b` (both corpora): `Rebound` and `Friction` — B1 `FUN_001f4608`
   104426 / 104343, `FUN_0027b640` 201470 / 201454; B2 `FUN_002263c0`, `FUN_0022dc20`, `FUN_0031a5f0`; never the
   sticky setters. 0x0d0cde2b — B1 `FUN_0027b640` 201464, `FUN_0027fb78` 204718; B2 `FUN_0031a5f0`, `FUN_00321960`.
   `hashloc.py StickPerson StickTerrain StickVehicle`: only `FUN_0027fb78` / `FUN_00321960`.
3. Twins: each B1 function above has its B2 function read, with the differences listed (variants, the rest rule, the
   launch point, the mine's contact, the grenade roll).
4. Constants: the bounce's 0.1 / 1.15 / 1.37 / 0.37 / 0.7 are the `lui` / `ori` pairs at 0x27f7f0, 0x27f814,
   0x27f828, 0x27f854, 0x27f8c4 (`eedis.py bf1 0x27f7b0 0x27f988`); the two grenade predicates and BF2's fire entry
   were read from the disassembly (`eedis.py bf1 0x28ede0 0x28ee90`, `bf2 0x337660 0x3377d0`).
5. Flags: the sticky "timer starts later" flag — default 1 (constructors B1 204643, B2 314388), one writer (the
   setter, key 0x0d0cde2b), on no disc class, 1 in RAM. `TriggerSingle` — base default 1 (B1 209496), setter B1
   210357; RAM 1 on the grenade, 0 on the rifle.
6. `re_check.py callers`: `FUN_00238678` <- `FUN_002387b0` only; `FUN_002ab288` <- `FUN_002ab590` only;
   `FUN_0028ef70` <- `FUN_0028ec38`; `FUN_00337b80` <- `FUN_003377d0`; `FUN_00236be8` <- `FUN_0027b428`,
   `FUN_0027f988`; the sweep / contact functions are vtable slots (0x53aa50 / 0x53aa54).
7. The verifier (Opus, claims and citations only, its own readers and scripts): 36 SUPPORTED, 6 PARTLY, 0 NOT
   SUPPORTED of 42 (`lead_tmp/gren/verify/VERDICTS.md`). Folded in: the forced-grenade alternative is a GROUP test,
   not a vehicle test (§7.2, both games — re-read here at B1 178852 / 183296, B2 233525 / 233894); the award gate's
   full rule and `FUN_00360018` (§2.4); `TriggerSingle` IS authored by the three BF1 mortar launchers (§3.1); the
   switch request lives on the Controllable, and four line ranges ran past their functions (§5.1, §7). Its own
   arithmetic on the live rows: launch error <= 1.2e-6 m/s; bounce residual <= 2.5e-6 m/s over 6 (BF1) + 10 (BF2)
   bounces with the other update order leaving 0.19-0.48; restitution measured 0.369999-0.370000; the count rule fits
   6/6 (BF1, floor normal) and 10/10 (BF2, slow bounce) and the other game's rule fits 2/6 and 6/10; rest -> gone
   0.901 s (BF1) / 0.600 s (BF2) of host time.

## 12. What stayed [H] or unread

- What the AI controller does with the grenade warning (vtable +0x30), and BF2's reaction-delay numbers.
- Who sets the medal bits of the award-weapon gate (§2.4) and which key shares an ammo object between BF2 slots.
- BF2 soldiers whose channel-0 weapon is a melee weapon route channel 1 to a third trigger word (149608-149645;
  heroes) — not followed.
- Keys 0x039975be (BF2 item class, a flag beside `LifeSpan`) and the grenade classes' byte 0x079c1d86 beyond "fire
  variant override" (BF1 mortar launcher 0, Ewok rock / spear 2, ALL mine dispenser 1; BF2 mortar launcher 0).
- BF2's dispenser `MaxItems` overflow path and the remote droid, auto turret and `destruct` weapons were not read.
- The crack grenade's stick, the mine and the detpack were not measured live (code and disc only).
- The pad buttons of the two switch inputs.

## 13. OPUS RE (2026-10-06) - [O] UNAUDITED — the BF2 throw beside the port's, and the terrain ray's hit point (card GRNP)

The maintainer, 2026-10-04: "the port feels like it is throwing grenades further than the original". Measured on the PS2
(PCSX2, SW renderer, no AI, one row per game update) and in the port from the same feet, aim yaw and aim pitch.
Scratch: `engine/artifacts/lead_tmp/grnp/` — `ps2/grnp_probe.py` (the PS2 throws), `tools/grnp_throw.rs` (the port's,
a scratch example on the real map with the mission's classes), `tools/compare.py` (the table), `tools/replay.py`
(a transcription of BF2's sticky + terrain ray), `ps2/out/*_throws.json` (the rows), `checks/` (re_check + verifier).

### 13.1 The throws [M]

BF2 Mos Eisley (the port's `tat2`), a level street (terrain y 0.000), feet (103, -0.02, 40) facing -X, Imperial
rifleman, thermal detonator; 15 throws: aim pitch 0.5 / 29.1 / 59.6 deg standing and running (7.0 m/s, the stick
forward from 1 s before the press), -26.3, -13.9 and the upper limit, 63.6 deg. Two throws each: equal row for row.

| aim pitch (deg) | first contact s, PS2 / port | rest: level m from the feet, PS2 / port | rest s, PS2 / port | blast s, PS2 / port | counted contacts PS2 / port |
|---|---|---|---|---|---|
| -26.3 | 0.233 / 0.233 | 12.81 / 13.73 | 1.467 / 1.267 | 2.067 / 1.867 | 7 / 8 |
| -13.9 | 0.467 / 0.467 | 22.98 / 24.15 | 1.967 / 1.800 | 2.567 / 2.400 | 9 / 11 |
| 0.5 (stand and run) | 1.100 / 1.100 | 35.49 / 36.57 | 2.467 / 2.233 | 3.067 / 2.833 | 8 / 9 |
| 29.1 (stand and run) | 2.633 / 2.633 | 51.90 / 52.02 | 4.200 / 4.000 | 4.800 / 4.600 | 6 / 6 |
| 59.6 standing | 3.767 / 3.733 | 40.60 / 40.59 | 5.933 / 5.700 | 6.533 / 6.300 | 7 / 6 |
| 59.6 running | 3.733 / 3.733 | 40.56 / 40.60 | 5.900 / 5.700 | 6.500 / 6.300 | 7 / 6 |
| 63.6 (the PS2's limit; the port stops at 63.0) | 3.833 / 3.833 | 36.41 / 37.12 | 6.033 / 5.867 | 6.633 / 6.467 | 7 / 6 |

- **The launch is the PS2's** [M]: the launch point is feet + (0, 1.8183, 0) + 0.7433 m along the facing at every
  pitch (the offset turns with the yaw only), the velocity normalise(aim + 0.21 y) x 25.000 — port and PS2 equal to
  four decimals in all 15; the grenade is released 16 updates after the press. **A running thrower adds nothing**:
  |v| = 25.000 at 7.0 m/s, in both.
- **The flight is the PS2's** [M]: until the first contact every port row is within 2 mm of the PS2's (one throw
  0.05 m: a one-row difference in when the contact is taken).
- **The bounces are not**: the port's grenade comes to rest 0.13-0.23 s sooner in every throw and, thrown level or
  downward, 0.9-1.2 m further (3-7 %); from 29 deg up within 0.12 m (the port's clamp at 63.0 deg adds 0.7 m).
  The fuse is equal: 18 updates from rest (or the stick) to the blast in both.
- The aim's upper limit is 63.62 deg on the PS2 (camera 53.01) against the port's 1.1 rad = 63.03; the PS2's
  camera looks 10.62-10.67 deg below the aim at every pitch, the port's 10.00.
- On a snow slope (BF2 HOT1, out of the hangar): rest 37.86 m / 3.033 s (PS2) against 37.84 m / 2.800 s (port).
- **The stick** [M] (BF2 HOT1, a parked snowspeeder 12.6 m ahead, aim -9.1 deg): the grenade stops on the hull on its
  13th update (velocity 0, the stuck record set, armed at once, the bounce count 0) and is gone 18 updates later —
  the same updates in the port (`Anchor::Vehicle`); the port's hull is met 0.81 m further along the throw (10.92 m
  from the feet on the PS2, 11.73 in the port: the hull's collision shape, not the grenade).
- **A BF1 map plays BF1's grenade** (§0): the port's `bf1_nab2` thermal detonator from the same aim leaves the fire
  point (feet + 1.425 m) 9 ticks after the press, rests at its 4th floor contact — 34.10 m at 1.867 s (level throw)
  and 50.82 m at 3.800 s (29 deg) — and burns 0.9 s. The maintainer's choice for the blended game is BF2's on every map.

### 13.2 Why the bounces differ: the terrain ray does not return the crossing [V, M] [O]

**Port progress guard, November 11.** The terrain marcher retains the original
split and tolerance for playable coordinates. It treats a rounded midpoint
that equals an endpoint as a leaf, preserves that leaf's height bracket, and
stops bisection when no new point is representable. A segment whose delta is
nonfinite is rejected. The midpoint sum uses half endpoints only when the
ordinary sum overflows. Focused fixtures retain the ordinary crossing,
vertical hit and terrain-cutter rejection. These guards address an engine
crash on malformed or extreme segments. They do not measure the map-specific
CPU spikes tracked by issue #110.

The sticky's sweep (B2 `FUN_00320de8` 313836-313919): first = the previous position (vtable +0x2c), second = the
position (+0x28), dir = normalise(second - first), origin = first - 0.1 dir, length = |second - first| + 0.2, mask
0x13ba (0x132a — no terrain bit — when the byte s+0x38 is 0); the point handed to the contact handler is
origin + dir x length x f, f the fraction the collision world returns (`FUN_00161c60` 1585 -> `FUN_00163078`
2587-2683). With mask bit 0x80 the world asks the terrain object (world+0xd0; vtable 0x61ed80, installed by
`FUN_002568a8` 173211, registered by `FUN_001619f8` 1428) through slot +0x5c = `FUN_00256b58` (ELF word at 0x61eddc):
*(decompiler excerpt omitted from the public copy)*
So a ray that is not vertical is cut at the FIRST MIDPOINT of a bisection that lies within 0.1 m of the terrain's
height, not at the crossing: a 1.058 m ray whose crossing is at 0.988 returns 0.75 (its midpoint at 0.75 is 0.080 m
over the ground) and the grenade bounces from 0.18 m up; another returns 0.5 with the midpoint 0.006 m UNDER the
ground. A hole test (`FUN_0055fdd0`) refuses a hit. `DAT_0060bed4` (the terrain is loaded: set by the `tern` chunk's
INFO, `FUN_0055db58` 694898, cleared by `FUN_0055eb50`) gates all of it. Every world ray with the terrain bit takes
this path — bolts too (`FUN_0055fb38` has two more callers, both in `FUN_00261208`, not read).
[M] `replay.py` — the sticky's update, sweep and bounce (§4.1-§4.3) over level terrain with this ray — reproduces all
15 PS2 throws row for row: 2,040 rows, worst position error 0.9 mm, every rest update and rest point. The same
transcription with the exact crossing instead rests 0.13-0.20 s sooner in 9 of the 15 and up to 0.64 m further (the
level and downward throws): the ray is the larger part of the port's difference; the rest is the port's own sweep
(its level throw makes 9 contacts where the PS2 and both transcriptions make 8).

### 13.3 Checks

`re_check.py fn b2` over the 18 cited lines (`lead_tmp/grnp/checks/fn_b2.txt`): each is in the function named;
`word b2 0x61eddc` = 0x00256b58; `callers`: `FUN_0055f818` <- `FUN_0055f940` only; `FUN_0055fb38` <-
`FUN_00256b58`, `FUN_00261208` x 2. The verifier (Opus, claims and citations only, `checks/verify_claims.md`): 9 of 9
SUPPORTED; folded in from it: the 3 m split measures dx^2 + dz^2 (0x55f988 `VMUL.xz`) while the bisection's stop is
3-D; mask 0x132a carries no terrain bit; the vtable's install and registration lines; the flag's default (0) and two
writers; `FUN_0055f940`'s shared flag byte. Its hand trace of the worked example returns 0.75.
Stayed [H] / unread: BF1's terrain ray (not opened — the port takes BF2's on every map); that `FUN_0055ebc0` is the
height and `FUN_0055fdd0` the hole test (by their use); the byte s+0x38; the callback 0x162f60 that tests the
bodies after the terrain (model floors: not measured here); the down limit of the aim.

### 13.4 The build (card GRNP, 2026-10-06, main fdff50d) [V, M]

- The port's sweep now takes this terrain ray (`core::collision::ray_terrain_ps2`) on every map; bolts and the camera
  keep the exact crossing.
- [V] The sweep's length is |second - first| + 0.2 FROM THE PULLED-BACK ORIGIN (B2 `FUN_00320de8` 313836-313919: the
  origin is first - 0.1 dir and `FUN_00161c60` gets *(decompiler detail omitted)*), so the ray ends 0.1 m past the new position. The
  port ran it 0.2 m past (+0.3): that was §13.2's "port's own sweep" difference — one more contact on the level throw.
- [M] The PS2 soldier's position is 0.020 m UNDER the terrain in all 15 throws (feet y -0.02 on terrain y 0: the kept
  2 cm of penetration, re-soldier-ground), and the launch point is 1.8183 over THAT; the port's feet stand on the
  terrain. From the port's feet the hand is 1.7983 m up (`Thrown::BF2_HAND`). 2 cm matters because the ray's cut is
  discontinuous: with 1.8183 the first contact came one update late in 5 of 7 throws and the rest was 0.05-0.2 m and
  1-3 updates off.
- [M] Result, the port on its own Mos Eisley from the PS2's feet and aim (`lead_tmp/grnp/ps2/out/
  tat2_a_compare_branch.txt`): 14 of the 15 throws rest at the PS2's point (to the table's 0.01 m) on the PS2's update
  with the PS2's count of contacts; the 15th differs only by the aim's upper limit (63.0 against 63.62 deg: 37.08 m
  against 36.41, the same updates). From each PS2 throw's own first row over level terrain the port is the PS2 row for
  row: 958 rows, worst 0.5 mm (`core/tests/data/grenade_ps2_bf2.txt`, test `sticky_ps2_bf2_throws_match_row_for_row`).
- From 2026-10-04 to 2026-11-01 a BF1 map's grenade took BF2's throw rules. The maintainer reversed that on 2026-11-01:
  a BF1 map throws BF1's grenade by BF1's numbers again (level, `bf1_nab2`: 34.10 m / 1.867 s; at 29.1 deg 50.82 m /
  3.800 s, the figures measured before the deviation). Only a BF1 class picked on a BF2 map still takes BF2's rules
  (`Thrown::throw_as_bf2`).

### 13.5 The bodies' ray is not pulled back - a named deviation (card GRFL, 2026-10-09, main b75d36b) [M], PROVISIONAL

- [M] the maintainer's playtest (BF2 Kamino, 2026-10-07): grenades fell through decks he stood on. The decks have an ordnance
  floor. The cause was the port's sweep: a bounce leaves the grenade 0.1 m off the face (`PUSH_OFF`), a steep impact
  leaves along the normal (4.3: f = 1), so the next ray, pulled back 0.1 m, starts ON the face; the port's mesh ray
  (parry) meets a face from either side and turns the normal toward the ray, so the face was hit from behind and the
  bounce put the grenade 0.1 m under the floor.
- The port's ray against the world's BODIES AND HULLS now starts at the previous position and ends where the PS2's
  does (length = step + 0.2 - 0.1); the terrain, water and soldier tests keep the pulled-back ray of 13.4, so the PS2
  rows of 13.4 (terrain) are unchanged.
- [H] What the PS2's mesh test does with a face behind or at its origin is NOT read (13.2 read the terrain only), and
  no PCSX2 throw at a deck was made. By its result the PS2's grenades rest on these decks. A one-sided mesh ray is no
  answer in the port: the baked collision meshes' windings are mixed (front faces only: 184,563 grenades through,
  back faces only: 90,947; card GRFL's prototype, main 5d83b7d).
- [M] The census (`game/examples/ordnance_census.rs`: 4 grenades on each of 389,228 soldier floors of the 32 maps):
  127,017 of 1,556,912 pass a floor a soldier stands on with the pulled-back ray, 7,260 without it. Of those 7,215
  cross a soldier-only body (6,140 the tops of Polis Massa's invisible walls, 935 the Kamino platform's skirt prim)
  and 45 a body with an ordnance layer (not traced). The maintainer's deck (103.1, 52.4, 75.0): 2 of 4 before, 0 of 4 after.

## Mines and detpacks (issue #43; merged from the contributor report `docs/loop/reports/20261028_MINE.md`, 2026-11-02)

Disc census: BF1 10 classes / 5 explosive weapon kinds, BF2 43 classes / 12 kinds. This comparison is executable-and-disc data
against the originals, not a live PCSX2 capture, and a visual and controller comparison is still outstanding.

- BF1 Alliance landmine: dispenser `MaxItems=4`; `TriggerRadius=1`; arms when landed; enemy velocity dotted with its
  displacement from the mine must be nonnegative. Trigger explosion damage 1750, inner/outer radii 4.5/6.5 m (BF1
  `FUN_0028d958`, `FUN_001f8410`, disc landmine and explosion classes).
- BF2 Alliance detpack: one live pack; charge limit 0.5 s, launch speed 2-10 m/s; the next fresh press triggers it. Pack
  health 20, lifetime 60 s; trigger explosion damage 3000, radii 0.5/3.5 m; destroyed and expired classes do no damage (BF2
  `FUN_003336a0`, `FUN_0022b170`, `FUN_0022bc58`, disc detpack and explosion classes).
- `[O] UNAUDITED`: FNV(`ImmuneToMines`) is `0x62a8f10f`; BF2 soldier setter `FUN_00252bb0` writes class+`0x8d8`, which the mine
  contact and update checks read. `python3 scripts/tools/eedis.py bf2 0x00253f58 0x00253f78` confirms the boolean parse and `sb`
  at `0x00253f70`. HealthType `Mine` hashes to `0x7e89edb6` and is category 7 in BF2 `FUN_0026a340`; `FUN_0030fc88` initializes
  all eight damage scales to 1. The port's ordinary health table has categories 0-6; placed items keep those when authored and
  use the original category-7 multiplier 1 for `Mine`, not VehicleScale.
- Port approximations, not measured PS2 parity: item collision is a PROVISIONAL 0.15 m sphere for hits and blast falloff; the
  terrain contact snaps to the heightfield after the approximate terrain ray, with bounce clearance capped at 1 cm; BF1 release
  uses the soldier fire-point approximation and BF2 the cached offhand offset; timer sounds use the bank's pitch, so the
  original rising tick pitch is not reproduced.
