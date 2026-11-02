# OPUS RE (2026-10-22) - [O] UNAUDITED
# RE: ordnance that is not a bolt - missile (guidance + the lock), shell, bullet, emitterordnance; and the bolt family's real ray model (card UORD)
AUDIT 2026-10-22 (loop lane 2): PARTLY - the ray model, the missile law, the lock, the shell and the emitter hold; the
bolt sweep is now measured live in both games (§2.4); fixed: IMP's torpedo waver (§4.1), what the live replay pairs
exercise (§0, §4.5), the mortar's burst height (§6), two wordings (§0 shell, §2.2 vt+0x34). Evidence
`engine/artifacts/lead_tmp/audit-uord/` (`ps2/`, `wf_result.json`).

Corpora: B1 = `analysis/SWBFRONT.ELF-decomp.c` (SLUS-20898), B2 = `analysis/SWBFRNT2.ELF-decomp.c` (SLUS-21240); line
numbers are of those files. Tags: [V] read at the cited lines, [M] measured on the PS2 (PCSX2 live RAM), [D] derived,
[H] hypothesis / unread, [O] load-bearing for the port. Anything not [V]/[M] is PROVISIONAL in code.
Scratch (gitignored): `engine/artifacts/lead_tmp/uord/` — `helpers/H1_bf2_missile.md`, `H2_emitter.md`,
`H3_bullet_shell.md`, `H4_lock.md` (the four readers' full notes with every line; this doc lifts their load-bearing
claims after the lead re-opened them), `ps2/` (the live probes, their JSON rows and replays), `checks/` (re_check
output), `verify/` (the workflow's verdicts).

## 0. The answers [O]

| question | BF1 | BF2 | tag |
|---|---|---|---|
| the bolt family | `bolt` / `laser` / `bullet` / `shell` / `missile` share ONE flight core (B1 `FUN_002764c0`, B2 `FUN_00313600`), one sweep and one impact handler; `bullet` = a bolt with a mesh and a light, no sim difference | same | V |
| what a bolt's rays test | ONE predictive ray at the first update against **buildings + terrain only** (mask 0x48), then a prev -> current SWEEP every frame against soldiers, vehicles, riders, water, foliage, shields (0x996); before the predictive ray has run the sweep also tests terrain + buildings (0x9de) | 0x90; sweep 0x132e / 0x13be | V, M (§2.4) |
| who skips the predictive ray | `missile` and `shell` (class flag 0): they sweep the full mask (terrain, buildings, units) every frame | same | V |
| missile flight | constant speed; each update turns the velocity toward a desired direction by min(ramp x TurnRate x dt, \|dir x desired\|) and wobbles it (WaverTurn, WaverRate); ramp = 0 -> 1 over the first second; no gravity | + a speed ramp MinSpeed -> Velocity at Acceleration (rocket launcher 20 -> 100 m/s at 80 m/s²); + a bot aim error on AI missiles at non-flyers; + a warning written to the target's occupants | V, M |
| desired direction | the target's aim point minus the missile while the target lives; else the LAUNCH direction (an untargeted missile steers back to its launch line); a lost target leaves the last direction | same | V, M |
| when a missile has a target | only when the firing weapon reports "locked" at the shot: a LAUNCHER after its dwell lock (LockTime: torpedo 1.5 s; rocket 1.0 s), consumed by each shot; any other weapon class (the wrist rocket, vehicle missiles) when it holds a target | same; a human's launcher takes the HUD lock target | V |
| what locks | torpedo / rocket launchers: vehicles only (TargetVehicle); bots lock whatever their combat target is when the class's kind bits allow it | rocket launchers: vehicles (human), + animals and buildings (bot AITarget*); bulldog: soldiers; EMP launcher: never (LockTime 0) | V |
| bots and the lock | the dwell runs for a bot too, but no AI fire path waits for it: a bot's missile homes only if its target was held ≥ LockTime before the aim-on press | same | V, D |
| shell | `Gravity` x −9.8 per update (re-grenades §4.1), full-mask sweep from the first frame, **explodes on the first non-foliage contact** (no bounce: `Rebound` / `Friction` on a shell are dead keys - read only by the `grenade` class and the item base (mine, powerupitem; BF2 also flag), and in BF2 the portable-turret class); life-out = ExplosionExpire in the air | same | V |
| BF1 mortar launcher | a `grenade` WEAPON, fires on release: a tap fires the shell `_ord1`; held past 0.9 of MaxPressedTime (0.675 s) the NextCharge stage, a bouncing sticky (`_ord`) | one stage, shell | V |
| emitterordnance | not a projectile: each weapon shot runs ONE instant cone query at the muzzle and chains damage target to target (MaxTargets, JumpDeduction, DamageThreshold, MaxJumpDistance), with LOS per hop | + team / alive filters, the class's damage scales, hit-all mode (the flamethrowers), burn damage over time, a blast per hit, pull | V |
| impact | deflection test -> direct damage -> effects -> foliage passes on, else the impact explosion (`Explosion` / `ExplosionImpact` / `ExplosionName`) at point + 0.2 x normal -> dies | same | V |
| life-out | `ExplosionExpire` at the final position (bullet, missile, shell alike) | same | V |

Live (§4.5): 1,400 update pairs of 25 missiles in the two games (8 BF1, 14 BF2 untargeted; 3 with a target knocked
into RAM) replay the transcribed law to ≤ 1.2e-4 m/s, one torn read excluded; the 3 targeted ones aim at the read
target point to ≤ 0.009°. AUDIT: not every pair tests the steering. 442 BF1 pairs never changed velocity (any
TurnRate fits them), and the 582 BF2 free-flight pairs never turn (desired = dir; every class in that capture has
WaverTurn 0), so they test only the speed ramp. The turn, waver and ramp terms rest on 263 + 27 BF1 pairs and 87 BF2
homing pairs, and BF2's waver is not measured live (mutations of the replay: `lead_tmp/audit-uord/mut/`).

## 1. Method

Hops: the ordnance ClassLabel tables (B1 `FUN_00172568`, B2 `FUN_00173758`) -> each class ctor (defaults, the ctor
chain), setter (hash -> field) and instance vtable (`re_check.py word`) -> the instance ctor (what it copies from the
spawn packet) and update -> the shared flight core, sweep and impact handler -> the weapon side: the shot spawn that
fills the packet (B1 `FUN_002892a8`, B2 `FUN_0032f208`), the launcher class and its lock, the AI's SetTarget and fire
path -> the target point (vt+0x5c / vt+0x58) -> the disc (every `ordc` / `wpnc`, `propscan.py`) -> live PCSX2 rows
(§4.5). Labels (B1 `re_check.py word b1 <va> --str`): beam, bullet, shell, grenade, sticky, beacon, haywire, laser,
bolt, missile, towcable, grapplinghook, emitterordnance, fatray; B2 the same set + meleethrowordnance.

| label | B1 class ctor | B2 class ctor |
|---|---|---|
| bullet | `FUN_00277248` (0xdc B) | `FUN_003148f0` (0x11c B) |
| laser / bolt | `FUN_0027c9d0` | `FUN_0031ba10` |
| missile | `FUN_0027dc78` (0xf4 B) | `FUN_0031fbe8` (0x13c B) |
| shell | `FUN_0027e598` | `FUN_003203f0` |
| emitterordnance | `FUN_00277d60` (0xbc B) | `FUN_003153a8` (0x16c B) |

Four readers (helper agents) read BF2's missile, the emitter, bullet / shell / shotgun and the lock in parallel; the
lead re-opened every line this doc marks load-bearing (§11) and ran the live checks.

## 2. The bolt family's flight and rays [V][O]

### 2.1 The class flag
The bullet class ctor sets the predictive-ray flag: B1 class+0x88 = 1 (`FUN_00277248` B1:198236), B2 class+0xc0 = 1
(B2:305151). The laser / bolt ctor IS the bullet ctor plus draw fields (B1 `FUN_0027c9d0` B1:202382, B2 B2:310035)
and never writes the flag. The missile ctor writes 0 (B1:203164, B2:313053), the shell ctor 0 (B1:203557, B2:313357),
fatray 1 (B1:199596, B2:307566). No setter writes it (H3 §1.1; the emitter's own +0x88 is another field, §8).

### 2.2 The core (B1 `FUN_002764c0`, B2 `FUN_00313600`)
- Spawn (B1 `FUN_002762a0`, B2 `FUN_003130d8`): velocity = packet direction x lerp(Velocity, NextCharge.Velocity,
  charge); the pending byte s+0x38 = 1 (B1:197536, B2:304026).
- First update, only `if (class+0x88 != 0 && s+0x38 != 0)` (B1:197629; B2:304258-304262 with +0xc0): ONE ray from the
  position, length \|v\| x life, **mask 0x48** (B1:197644; B2 0x90 B2:304292); a hit cuts life to the hit time and
  keeps the body (B1:197646-197653); then s+0x38 = 0 (B1:197656, B2:304304) — the only store of 0, inside the branch.
- Every update: pos += v x min(life, dt); life via the base `FUN_00272e98` / `FUN_0030f988`; when life runs out with
  a kept hit whose owner id still matches, the impact handler runs (B1:197723-197731).
- **The sweep** (instance vt+0x38, B1 `FUN_00273128`, B2 `FUN_0030fad0`, shared by every class of the family):
  called once a frame by the collision manager for every moving ordnance (B1 `FUN_00163198` B1:2731-2738: vt+0x38,
  then vt+0x34 when it returns 0 - the bullet's destroys it, the laser's `FUN_0027c410` unlinks it from the list and
  lets its beam tail drain (AUDIT); the list is filled only through `FUN_00272d50`, called by 11 ordnance creators,
  bullet and laser among them); a ray from the previous to the current position (vt+0x2c -> vt+0x28),
  owner ignored, **mask 0x9de while s+0x38 is set, 0x996 after** (B1:195510-195514; B2 0x13be / 0x132e,
  B2:301923-301925); a hit calls vt+0x3c, the impact handler (B1:195532-195534).
- [D] 0x9de − 0x996 = 0x48: after the predictive ray the sweep no longer tests buildings (8) and terrain (0x40) but
  keeps soldiers 2, vehicles 4, riders 0x10, water 0x80, foliage 0x100, shields 0x800 (layer ids:
  re-collision-layers §5, re-turret-collision 3.3). **So a soldier who steps out of a bolt's line is missed and one
  who steps in is hit.** This corrects `re-blaster-damage.md` §5.1 ("one predictive ray ... NO further collision
  testing ... targets CANNOT dodge") and the port's `core/src/projectile.rs` header and test
  `predictive_raycast_hits_dodgers_and_ignores_late_entrants` (`core/src/sim/combat.rs:739`), which assert the
  opposite. [V code; M live in both games, §2.4]
- `missile` and `shell` (flag 0) never run the predictive ray, so s+0x38 stays 1 and their sweep keeps mask 0x9de /
  0x13be the whole flight: terrain, buildings and units, one chord per frame. [V; the missile rows of §4.5 keep
  b38 = 1 in every row (M)]

### 2.3 Instance vtables [V] (`re_check.py word`)
| slot | B1 bullet 0x539c08 | B1 missile 0x53a6f8 | B1 shell 0x53a890 | B2 bullet 0x626960 | B2 missile 0x627648 | B2 shell 0x627808 |
|---|---|---|---|---|---|---|
| +0xc update | `FUN_002764c0` | `FUN_0027d6b8` | `FUN_0027e478` | `FUN_00313600` | `FUN_0031f2e8` | `FUN_003202d0` |
| +0x38 sweep | `FUN_00273128` | same | same | `FUN_0030fad0` | same | same |
| +0x3c impact | `FUN_00276810` | same | same | `FUN_00313c40` | same | same |
The missile and shell updates end in the bullet core (B1:203144, B1:203516; B2:313031).

### 2.4 Live on the PS2: units are struck by the sweep, never by the predictive ray [M] (AUDIT 2026-10-22)
Probes `lead_tmp/audit-uord/ps2/bolt_b1.py` / `bolt_b2.py` (every bolt-family instance by its vtable word at +0x14:
B1 laser 0x53a568 / bullet 0x539c08, B2 laser 0x6272d0 / bullet 0x626960; one row per update, the last read of each
`life`), readers `passive_read.py` / `knock_read.py`; the UORD states `b1_kas2_arc_spawned.p2s` /
`b2_kas2_heavy_spawned.p2s`, SW renderer, AI battles.
- **Passive** (`b1_passive1.json` 30 s, 257 bolts; `b2_passive1.json` 40 s, 477): s+0x38 is 0 at the first row seen
  in every bolt (BF1 238, BF2 363). 15 BF1 and 18 BF2 bolts stopped with 0.09-1.3 s of life left, the last step's
  chord passing within 0.82 m of a soldier's chest (feet + 1 m; one BF2 at 1.25 m). For each of them the life at the
  first row put the end point 10-104 m (BF1) and 17-171 m (BF2) PAST that soldier: the predictive ray had not cut
  them there, the sweep stopped them. The owner of the predictive kept hit (B1 s+0xf4, B2 s+0x134) is a soldier in 0
  of 661 BF1 bolts with a kept hit (627 world, 34 three non-soldier entities; the tracked bolts of `b1_knock1.json`)
  and 0 of 409 BF2 ones (354 world, 55 five non-soldier entities, their classes read in RAM).
- **Knock-in** (`b1_knock2.json`, `b2_knock1.json`): at a bolt's first row (s+0x38 = 0) the probe wrote its prev and
  position half an update-step before a stationary soldier's chest and its velocity through the chest (|v| kept,
  life 0.5 s, kept hit and ended cleared); every other knock moved the same line 2.5 m sideways. Through the chest:
  BF1 15 / 15 and BF2 15 / 15 stopped one update later, 0.17-0.59 m short of the chest centre, with 0.46 s of life
  left; the soldier lost health in 14 / 15 and 15 / 15 (BF1 40-60, BF2 60-100). Offset 2.5 m: BF1 14 / 15 and BF2
  13 / 15 flew on to life-out 36-145 m past (one BF1 and one BF2 bolt struck another body on the offset line, one
  BF2 write raced an update). So a soldier who steps into a bolt's line after the shot is hit.
- Two knock-ins of the first BF1 run (`b1_knock1.json` knocks 4 and 6) aimed at a soldier who then moved 0.5-0.9 m
  before the step: both flew on to life-out with no damage - a soldier who leaves the line is missed.

## 3. Impact and expiry [V][O]

The impact handler (B1 `FUN_00276810` B1:197737-198104, B2 `FUN_00313c40` B2:304466-304892), in order (H3 §2):
1. Owner of the struck body; a part code (−1; 1 = a shield body; 2 = a small sub-body) [H meaning].
2. Deflection: owner vcall +0xd0 (B1:197848) / +0xdc (B2:304598); accepted -> a new reflected ordnance, the original
   dies (re-blaster-damage §5.2).
3. Direct damage: descriptor `FUN_00272f88` (B1:197920) / `FUN_00310178` with the age falloff (B2:304603).
4. Effects and decals by the struck layer (B1:197944-198019).
5. Base collision (B1 `FUN_00272d70` B1:198103, B2 `FUN_0030f550`): foliage (B1 0x100, B2 0x200) returns 0 — the
   ordnance flies on (B1:195309-195310); else the impact explosion (class+0x38 / +0x4c: `Explosion` 0x02bb1fe0,
   `ExplosionImpact` 0x61a8bb12, `ExplosionName` 0x9b30e763; base setter B1 `FUN_002738a8`) at point + 0.2 x normal
   (B1:195316-195325), then it dies. (The port's RKEX built this explosion.)

Expiry (base B1 `FUN_00272e98` B1:195346-195374, B2 `FUN_0030f988` B2:301804-301833): life -= dt while s+0x39 is set;
at ≤ 0 the `ExplosionExpire` class (B1 class+0x34, B2 +0x48) spawns at the position (B1:195365-195367) — for bullet,
missile and shell alike, with no test of a kept hit. [D] A bolt/bullet whose predictive ray hit terrain has its life
cut to the hit time, so it would fire ExplosionExpire there and the impact explosion one frame later (BF1's jet
trooper rocket authors both) — not seen live [H].
[M] The BF1 torpedo class in RAM: ExplosionExpire (class+0x34) = 0, impact explosion (+0x38) set: a torpedo that runs
out of life just vanishes.

## 4. The missile [V][M][O]

### 4.1 Class keys
| key (hash) | B1 off | B2 off | ctor default | disc |
|---|---|---|---|---|
| LifeSpan 0x7c7c544b | +0x30 | +0x30 | 3.0 | torpedo 8 (BF1), rocket 10 (BF2 RAM) |
| Velocity 0x32741c32 | +0xac | +0xe4 | 35.0 | torpedo 40, Wookiee 80; BF2 rocket 100 |
| TurnRate 0x0f77c4e8 | +0xe0 | +0x120 | 1.0 | torpedo 2.0; BF2 rocket 2.0, bulldog 0.15, wrist rocket 0 |
| WaverRate 0x6d4a8840 | +0xe4 | +0x124 | 0 | REP / ALL torpedo 0.5; the IMP, CIS and Wookiee torpedo classes author neither Waver key (AUDIT: `entc.py` on SIDE/IMP.LVL lists 18 PROPs for `imp_weap_inf_torpedo_launcher_ord`, REP / ALL 20 with both keys at 0.5); BF2 rocket 0 |
| WaverTurn 0x129f08c1 | +0xe8 | +0x128 | 0 | as WaverRate |
| 0x73e5a988 (unnamed) | +0xdc = 1 / value | +0x11c = 1 / value | 1.0 | 0 PROP chunks on either disc |
| LockedOnSound 0x9371aecd | +0xec | +0x12c | — | 0 PROP chunks on either disc |
| MinSpeed 0xfbe2eb62 | — | +0x134 | 0.1 | BF2 rocket 20, bulldog 20 |
| Acceleration 0x2fa0ba9d | — | +0x138 | 10000 | BF2 rocket 80, bulldog 60 |
Setters: B1 `FUN_0027ddb0` (B1:203203-203243), B2 `FUN_0031fd68` (B2:313095-313143); else the bullet setter. Defaults:
B1 ctor `FUN_0027dc78` B1:203149-203169, B2 `FUN_0031fbe8` B2:313036-313056. [V] Names: WaverRate / WaverTurn /
TurnRate / LockedOnSound by FNV against the mod-docs vocabulary; MinSpeed / Acceleration (H1 §2). B2 quirk: MinSpeed
falls through and also writes +0x138; every disc class authors MinSpeed before Acceleration, so it changes nothing (H1).

### 4.2 The instance (B1 `FUN_0027d3f0` B1:202810-202867; B2 `FUN_0031ed70` B2:312386-)
- Target s+0x120 = packet+0x38, its handle s+0x124 = target+0xb4, part s+0x128 = packet+0x3c (B1:202828-202834); B2
  s+0x160 = packet+0x44, handle target+0x230, part s+0x168 = packet+0x48 (B2:312410-312416).
- Two waver phases = U[0,1) x 2π each, from the gameplay LCG (B1 DAT_0066eed0, B2 DAT_0073d870).
- Desired direction = packet+0x10, the shot direction after spread (B1 s+0x110; B2 s+0x150 and the unit direction
  s+0x180, B2:312426-312440). Ramp s+0x12c / s+0x16c = 0.
- B2 only: speed s+0x194 = Velocity if Acceleration > 9000, else MinSpeed (B2:312441-312447; 9000.0 at 0x31ef2c);
  velocity = dir x speed (B2:312448-312452), top speed s+0x190 = Velocity (B2:312456). [V, opened by the lead]

### 4.3 The update (B1 `FUN_0027d6b8` B1:202890-203147; B2 `FUN_0031f2e8` B2:312606-313031)
```
r     = min(1, r + dt * class.rampk)                    # rampk = 1 / 0x73e5a988 = 1.0 on every class
turn  = r * lerp(TurnRate, NextCharge.TurnRate, charge)
if turn > 0:
    if target valid (handle check):                     # else drop it; nothing else drops a target
        aim     = BF1: target.vt+0x5c(target, pos, dir, part)
                  BF2: target.vt+0x58(...) + target.vt+0x5c(target)  (+ the bot aim error, below)
        desired = normalize(aim - pos)
    axis = dir x desired                                # dir = BF1 normalize(vel), BF2 s+0x180
    if |axis|^2 > 1e-8:  rotate dir about axis/|axis| by min(turn * dt, |axis|)      # |axis| = sqrtf
w = r * lerp(WaverTurn, NextCharge.WaverTurn, charge)
if w > 0:
    p1 = normalize(Y x dir);  p3 = n x p1               # n = normalize(dir); Y = (0, 1, 0)
    v  = sin(phase1) * p1 + sin(phase2) * p3            # table sine
    if |v|^2 > 1e-8:  rotate dir about v/|v| by |v| * w * dt
    phase1 += 2 dt lerp(WaverRate) * U1;  phase2 += 2 dt lerp(WaverRate) * U2       # U fresh LCG draws
BF2: speed = min(speed + Acceleration * dt, Velocity);  vel = normalize(dir) * speed
then the bullet core (move, life) — no gravity anywhere
```
Cites [V]: B1 ramp and turn B1:202941-202947; target check B1:202955-202963; aim and desired B1:202965-202979; axis,
sqrt (`FUN_00435b30` = sqrtf), clamp and rotation (`FUN_00446050` = Rodrigues from the table sincos) B1:202982-203013;
waver B1:203015-203108 (up = … bullet core B1:203144. B2: ramp
B2:312658-312663, target check B2:312665-312672, aim B2:312676-312683, steering B2:312842-312872, waver
B2:312881-312984, speed B2:312985-312988, velocity B2:312990-313005, core B2:313031 (lead re-opened the ramp, target,
speed and velocity lines).
- [D] The clamp is against \|axis\| = sin θ, not θ: at TurnRate 2 and 30 Hz the cap matters only within ~3.8° of the
  desired line or past ~176° (a target straight behind gives no turn).
- [D] The steering runs with or without a target. With none, desired = the launch direction, so the waver cannot
  build a drift: the missile wobbles about its launch line.
- [V] Lock loss: only the dead-handle test (B1:202955-202963, B2:312665-312672) — no cone, range or LOS test in
  flight; with TurnRate 0 the block that reads the target (inside `turn > 0`) does not run.
- [V] BF2's bot aim error: when the missile's owner is AI-controlled (a UnitController) and the target is not an
  EntityFlyer, `FUN_002a4cc8`'s aim-error vector (re-soldier-ai 17.2, with no range factor) is added to the aim point
  (B2:312711-312752; H1 §4). BF1 has none (B1:202965-202979).
- [V] BF2's warning: in the ctor and each turning update with a valid target, every occupant controllable of the
  target (target+0x240 list) gets c+0x10c = 2.0, c+0x110 = now, and the nearest missile and its distance in
  c+0x114 / +0x118 (B2:312477-312541, 312781-312840); the dtor clears them. Its reader is not found [H]; the HUD's
  `player%d.missileLock*` events are the likely one (B2 `FUN_003b0aa8`) [H].

### 4.4 The target point [V] (H4 §5)
- BF1 soldier: vt+0x5c = `FUN_0020c680` = the soldier matrix (O+0x200) x (0, h, 0), h = 1.4 / 0.6 / 0.2 by stance
  (table 0x586350). [M] §4.5: the desired direction equals normalize(feet + 1.4 x up − position) to 0.000°.
- BF1 entities (21 vtables): `FUN_001e2500` — part ≥ 0: that body; else the TargetableCollision body best aligned with
  the missile's direction; with none, the collision-sphere centre (B1:91606-91683). BF2: the soldier `FUN_0024db90` =
  matrix x class offset (class+0x1280 + 16 x stance, (0, 1.4, 0) standing); entities `FUN_00213488` (+ a class offset
  +0x2d0, key unread [H]); vt+0x5c = zero except three flyer vtables (`FUN_0020ead8`, a lead on the flyer's motion
  [H]).

### 4.5 Live on the PS2 [M]
Probes `lead_tmp/uord/ps2/missile_b1.py`, `missile_b2.py` (one row per game update of every live missile, by the
instance vtable word at +0x14; the last clean read of each `life` value), replays `replay_b1.py` / `replay_b2.py`
(the transcribed update, with the class's RAM values, predicting update n+1's velocity from row n's velocity and
phases and row n+1's ramp and desired direction), `homing_knockin.py` + `homing_check.py`. SW renderer, AI battles:
BF1 KAS2 Clone Wars as the ARC trooper (`ps2/out/b1_kas2_arc_spawned.p2s`), BF2 KAS2 Clone Wars as the heavy trooper
(`b2_kas2_heavy_spawned.p2s`); the player's own launcher (R1) and every bot's missile in flight.
- **BF1, untargeted** (`b1_m_free.json`, 8 missiles, every one with target 0, the player's and the bots'): the four of
  the REP torpedo class (\|v\| 40, TurnRate 2, WaverRate 0.5, WaverTurn 0.5 in RAM, class flag +0x88 = 0) fit
  the law to ≤ 1.1e-4 m/s over 263 update pairs, against 0.014 m/s mean for "velocity unchanged"; the same law with
  the rotation reversed misses by 0.03 m/s. The two 40 m/s and two 80 m/s missiles that never changed velocity fit
  WaverTurn 0 exactly over 442 pairs (the CIS and Wookiee torpedo classes author no Waver keys; their class is
  inferred by speed, not read [D]); their desired direction = their launch direction. 705 pairs in all. AUDIT: those
  442 pairs never changed velocity, so they fit any TurnRate and show only WaverTurn 0. The age
  field +0x104 of the player's missiles reads 0 every other update (something resets it; rows are keyed on life)
  [unexplained, H].
- **BF2, untargeted** (`b2_m_free.json`, 14 missiles: player and bot rocket launchers, bulldogs, a vehicle missile
  class with TurnRate 0): 580 of 582 update pairs fit to ≤ 1e-4 m/s (566 to ≤ 4e-5 with the life difference as dt;
  AUDIT, `replay_b2.py`'s own step); speed 20 + 80 x dt per update to 100 (rocket), 20 + 60 x dt (bulldog; its cap 80
  is not reached in the rows), constant 80 (Acceleration 10000 > 9000; the rows cannot tell this from a clamp on the
  first update). The two misses are one torn row (speed already written, life not yet). No pair turns (desired =
  dir, WaverTurn 0 in all four classes): these pairs test the speed law only. The vehicle missiles carried targets (from their cannon-class weapon) and never turned:
  TurnRate 0.
- **Homing, knock-in** (`b1_homing.json`, `b2_homing.json`): at a new missile's first sighting the probe WRITES a
  target (an alive soldier 42-52° off the aim at 65-110 m; part −1) into the three fields the ctor fills from a lock.
  BF1 (1 missile, 27 pairs): law ≤ 1.0e-4 m/s; desired = normalize(target feet + 1.4 up − position before the
  update) to 0.000° with the target's PREVIOUS-update matrix (0.23° with the same update's: the missile updates
  before the soldier); turned 40° in 0.9 s before it struck the scenery. BF2 (2 rocket missiles, 87 pairs): law ≤
  1.0e-4 m/s; desired within 0.009° of the target's point (feet + the class offset (0, 1.4, 0)); each turned 63° and
  ended 0.6 / 1.5 m from the target point (hits). So the steering law, the target point and the handle re-check are
  measured; the lock that fills the fields in play is read code (§5), not measured.

## 5. The lock: when a missile gets its target [V][O] (H4)

- The packet's target is filled only when the weapon's vt+0x34 says so: B1 packet+0x38 = W+0x128 and +0x3c = W+0x130
  `if (W.vt+0x34(W) != 0)`, else 0 / −1 (B1:211797-211811, opened by the lead); B2 +0x44 / +0x48 = vt+0x38 / vt+0x3c
  under the same test (B2:324152-324168).
- Only the launcher class overrides vt+0x34 with a LOCK: it returns the dwell-lock flag (B1 `FUN_002909f0` = W+0x168,
  B2 `FUN_0033af18` = W+0x17c; each referenced only as its launcher vtable's word, no `jal` [receipt:859265a3]).
  15 other BF1 weapon vtables return "holds a valid target" (`FUN_00288168`, B2 `FUN_0032b4e8`); the repair weapon
  (vtable 0x53c870, ctor `FUN_00292740`) has its own target test `FUN_002927d8` (verifiers, §11.1).
- The dwell (B1 launcher `FUN_00290680`, setter `FUN_00290758`: LockTime 0x448fbce3 -> W.class+0x250, B1:217008;
  missile-ordnance flag class+0x258 B1:217002-217004): the weapon's charge state 4 is entered with no button while the
  weapon holds a target (`FUN_002905e0` B1:216900-216915; with LockTime ≤ 0 it falls back to the base charge test
  `FUN_00289110`); the clock limit W+0xd4 = LockTime, or the class's +0x1e0 when LockTime ≤ 0 (`FUN_002902f8`); the
  tick (`FUN_00290408`, skipped unless class+0x258) adds dt and sets the flag at the limit (B1:216792-216796, opened by
  the lead); a new target or part restarts it (B1:216808-216827); the exit-fire hook clears it when class+0x258 AND
  class+0x254 are set (B1:216713-216715; +0x254 defaults to 1 and its key 0xc61c7ccc is on no disc class, so every
  shot consumes the lock). Firing before the lock = an unguided missile.
- BF2's hooks are the twins at the top (`FUN_0033a840` / `FUN_0033a950` / `FUN_0033aa58` / `FUN_0033a620` /
  `FUN_0033ab60`; tick B2:331859-331863, exit-fire B2:331661-331663 with class +0x305 && +0x304) but differ after
  (both verifiers, §11.1): BF2 takes its candidate from `FUN_0033a658` (a human: a fresh pick, below; a bot: W+0x10c /
  +0x114) and, with NO target, always resets the clock and the flag; BF1 keeps dwelling while its stored target is
  still null and the part unchanged, so a BF1 launcher can raise the flag on nothing (harmless: the packet's target is
  then W+0x128 = 0, an unguided shot). BF2 stores the dwell target as pointer + generation (W+0x170 / +0x174).
- Who holds the target: BF1 human = the soldier magnet (`FUN_0025e098`, re-aim-assist §3: LockOnRange + r, the
  LockOnAngle cone, then the LockOffAngle cone once locked, LOS, team and Target* kind); BF1 bot = AI SetTarget
  `FUN_0023d1f8` (W+0x128, part −1, team / kind bits only, B1:155443-155449). BF2 human = the HUD lock target
  (player+0x928, acquired by `FUN_0017a908`, TargetLockMaxDistance 100 / ...Lose 110 m defaults) ∩ Target* ∩ a 25.8°
  cone of the aim ∩ LOS (B2:331689-331750); BF2 bot = `FUN_002bbb98` (W+0x10c, AITarget* kinds, part −1,
  B2:242514-242534).
- **Bots never wait for the lock:** the AI trigger (B1 `FUN_0023cd40` B1:155149-155257, B2 twin) does not read the
  flag; its one launcher test keeps a launcher out of the charge-hold branch (B1:154927, B2:242112). [D] A bot's
  missile homes only when its target was held ≥ LockTime before the aim-on press; follow-up shots are unguided. The
  share in play is not measured [H] — in the §4.5 battles no bot infantry missile carried a target (6 BF1, 7 BF2:
  5 rocket launcher, 2 bulldog); what each bot was aiming at was not logged.
- Disc (H4 §4): BF1 torpedo launchers LockOnRange 175, LockOnAngle 6°, LockOffAngle 10°, LockTime 1.5, vehicles only
  [M: RAM weapon class +0xd4 = 175, +0xd8 = 0.105, +0xdc = 0.175]; BF2 rocket launchers 250 / 6 / 10 / 1.0, vehicles
  (AITarget: animal, vehicle, building); BF2 EMP launcher LockTime 0 (never homes); bulldog 40 / 1 / — / 0.4, soldiers;
  the wrist rockets are cannons (LockTime is parsed only by the launcher setters, B1 `FUN_00290758` B1:217008, B2
  `FUN_0033acc8` B2:332061 [receipt:5adcaf40]). BF1's `rep_weap_inf_rocket_launcher` is a cannon firing
  a `bullet`: no homing.
- `re-vehicles.md` corrections (H4): BF1's lock HUD square is B1:262320-262339 (B1:262211 is the refire alpha); BF2's
  Controllable ticks copy TargetLockMaxDistance / ...Lose and the Target* mask, not LockOnRange / Angle / OffAngle.

## 6. The shell and the mortar launcher [V][O] (H3 §3-4)

- Shell update (B1 `FUN_0027e478` B1:203497-203542, B2 `FUN_003202d0`): v.y += Gravity x −9.8 x dt above water, then
  the bullet core (re-grenades §4.1). Flag 0 (§2.1) -> full-mask sweep every frame. Impact handler = the bullet's
  (vtable +0x3c `FUN_00276810`, `re_check.py word b1 0x53a890`): on the first non-foliage contact direct damage +
  ExplosionImpact at point + 0.2 n, then it dies. **No bounce**: the shell setter (B1 `FUN_0027e668`
  B1:203592-203598, B2 `FUN_003204c0`) reads only `Gravity`; Rebound / Friction are read only by the `grenade` class
  and items [receipt:b28f880b]. (`re-blaster-damage.md` §7.3 lists Rebound under the shell: wrong.)
- BF1 `imp/cis_weap_inf_mortar_launcher` = label `grenade` (B1:15472): MaxPressedTime 0.75, fires on release; its
  class has a NextCharge stage at 0.9: the weapon's current class moves to the next stage when pressed / MaxPressedTime
  > the stage's threshold (`FUN_00285108` B1:208951-208961, opened by the lead; the charge-stage step every
  charged weapon class shares - 5 `jal` callers, 8 vtable words - and it can advance several stages in one call),
  so a tap fires `_ord1` (shell:
  Velocity 40, Gravity 1.75, LifeSpan default 3.0, Damage 150) and a hold past 0.675 s fires `_ord` (a sticky:
  bounces, 0.5 s fuse). ALL's mortar has the sticky only. [D] A 45° lob of `_ord1` needs 3.3 s to land (93 m on the
  flat): at 3.0 s it bursts in the air with ExplosionExpire - ~6.8 m up at 85 m with the code's 30 Hz steps (v, then
  position; 7.7 m is the continuous solution; AUDIT).

## 7. The bullet: BF1's jet trooper, BF2's shotguns and chaingun [V] (H3 §4-5)

- BF1 `rep_weap_inf_rocket_launcher` (the jet trooper's "EMP launcher"): label `cannon`, TriggerSingle, one rocket per
  press, ordnance `bullet` (Velocity 40, LifeSpan 8, Damage 120, PersonScale 0.5, VehicleScale 0, both explosion keys).
  It flies as a bolt; its Gravity / Rebound / Friction / Waver / Stick / Target keys are dead.
- BF2 shotguns (`com_weap_inf_shotgun` 8 pellets, award 7): label `cannon`, all pellets in ONE update, each with its
  own triangular spread (±3° x ±4.5°), one round each; pellet `bullet` 2000 m/s, 0.1 s, 65 -> 15 by the age falloff.
  A 0-damage tracer bolt rides with every pellet: `SecondaryOrdnanceName` 0xdddb253a, `SecondaryOrdnancePeriod`
  0x82e86218 = 1 (B2:324171-324181). Chaingun: automatic 20 / s, 20000 m/s, a tracer on rounds 1, 6, 11, ...
- The secondary rule [V] (2026-10-30, `FUN_0032f208` B2:311910-311965): the weapon class keeps the secondary class at
  +0x2ec and the period at +0x2f0; `carries = counter++ % period == 0`, so the FIRST round carries it, and the round is
  made through the secondary class's own create slot (vt+0x10), so any ordnance kind rides (the sonic blaster's
  `fatray`, the geonosian turret's hidden bolt), not only 0-damage tracers.
- BF2's falloff age grows once per update, and a pellet struck in its first update is still age 0 [M]: PCSX2 on Dagobah
  (2026-10-28, Rebel engineer vs a stormtrooper held 3 m ahead, health reset before each shot) read every shot as a
  multiple of 65: 520 = 8 body, 455 = 7 body, 975 = 3 body + 4 head, 1430 = 1 body + 7 head, 1560 = 8 head. So a
  point-blank pellet does 65 to the body and 195 to the head (x3), and one body shot (520) kills a 400-health soldier.
  The earlier deduction (the age already one update old at the first strike, 48.3) was wrong by one step. [D] the steps
  are then 65 / 48.3 / 31.7 per 66.7 m; only the first is measured (the 10 m and 25 m runs lost their aim). Probe:
  `lead_tmp/sgprobe/shotgun_live.py` (not kept in git). The port: `Bolt::advance` counts only the updates a bolt came
  through.

## 8. The emitter [V][O] (H2)

Not a projectile: the class's create slot (vt+0x10, B1 `FUN_00278208` B1:198836-199273, B2 `FUN_00315f78`
B2:305952-307007) runs one query per weapon shot and returns 0 (B1:199272).
- BF1: gather box half-side ConeLength L centred at muzzle + 0.9 L x aim, mask **0x812** (soldiers, riders, shields —
  no vehicles), ≤ 32 (B1:198922-198940, opened by the lead); cone test at each body's origin (tan θ < sin(ConeAngle/2),
  t < L; no front test); nearest along the aim first. D = lerp(Damage, MaxDamage, charge) (B1:199074-199075). Pop: the
  axial step from the last hit must be ≤ ConeLength (first) / MaxJumpDistance (B1:199111-199118); LOS from the chain
  origin to the body + 1 m, mask 0x4c (B1:199131); FirstRadius on the first only; damage with **every type and shield
  scale forced to 1.0**; push; lightning; D −= JumpDeduction; stop at MaxTargets or D < DamageThreshold. The chain
  origin moves to the struck body only when a LightningEffect instance exists (always after hit 1, then with p 0.7).
  No team or alive test. LightningEffect 0x6dd5a894 is the key that writes the emitter's class+0x88 (B1:198741-198744).
- BF2 adds: mask 0x102a (vehicles too), 3-D distance and a true cone at the sphere centre (widened to cos 0.1 within
  2.5 m), the alive bit, AffectFriends / AffectEnemies, the class's scales, a hit-all mode (0x4c387ca1: D = max(Damage,
  D/N), no chain range) for the flamethrowers, deflection by the target's weapon, pull (Push < 0), a blast per hit
  (`Explosion`), buffs, and burn: soldier timer = max(timer, T), rate x dt per update while alive, scales 1.0.
- Fielded: BF1 clone pilot `rep_weap_inf_arccaster` (L 30, 50°, FirstRadius 0.6, MaxTargets 4, D 100 -> 250 by charge,
  JumpDeduction 75, DamageThreshold 75, MaxJumpDistance 20): 1 target below charge 1/3, 2 below 5/6, 3 above; 4 is
  unreachable. BF2 dark trooper arc caster (1 + ⌊5f⌋ targets, 300 + 25f, Push 3), the Jawa arc caster, the officer's
  incinerator (30 per shot, 15-shot burst, hit-all) and the hero flamethrowers (5 per shot + burn 50/s for 5 s).
- Burn keys [V] (2026-10-30, card UORDE): the setter `FUN_003156e0` (B2:293400-293680) stores `0x6824f471` (time,
  class+0x130), `0x1076dfd6` (rate, +0x160), `0x6cf895fb` / `0x592fb160` (bone / effect, 5 slots at +0x138 / +0x14c),
  `0xc0e20834` (the shot effect started on the shooter, +0x12c) and `0x348506dc` (a bool, +0x134 bit 0, no reader
  found); `FUN_00236e28` sets the soldier's timer +0x105c = max, overwrites the rate +0x1064 and the desc +0x1068;
  `FUN_00236f50` runs it from the soldier update. The incinerator authors no rate (ctor 0): a mark, no damage.

- Miss lightning [V] (2026-10-30): BF1 draws ONE bolt when the query struck nothing and a lightning class exists
  (B1:199011-199012), from the muzzle to the aim ray's end within ConeLength (mask 0x4c); BF2 draws NumBolts
  (0x63384b21, ctor 1) less the hits. BF2's bolt spread (0xdd09f94e, hero classes only, global RNG) is not modelled.

### 8.1 BF2 hero emitter clarification

Fresh B2 source/instruction reads for the hero consumer distinguish several fields
which must not be merged. FUN003153a8 defaults range10, first-radius1.5,
max-jump10, max-targets5 and zero deduction/threshold/hit-all. Cone membership is
strict distance>0 and<range, strict dot>cos; inside2.5m the cosine is
min(authored,0.1). The heap key is actual 3D distance: MIPS31647c supplies
squared distance in f12 and316508 calls sqrt0052bc10. The later chain-distance
difference therefore cannot use squared distances.

Ordinary damage effects use class+130(hash6824f471) and+160(1076dfd6),
not the separate timed debuff damage fields+120/+124. FUN00236e28,
150887–150940, keeps max(timer), always replaces rate/attribution and releases
previous visual instances. FUN00236f50,150949–151046, decrements the timer
before testing it, damages only while remaining>0 and alive, and uses eight
1.0 type scales plus1.0 shield scale. Lightning authors3s with0DPS to carry
its electrical damage particles; hero flame authors5s with50DPS. The ordinary
burn has no separate hero exclusion. The timed debuff path's hero exclusion
must not be imported into this one.

Damage-effect bone names(hash6cf895fb) and effect names(592fb160) append to
independent five-slot arrays atclass+138/+14c. They are paired by slot,
not by adjacency in file order. Native positive push's apparent two-float
call loses an implicit third argument:316bb8..316c00 preserves Push in f14
while f12=.5L and f13=2L. Negative pull uses a virtual origin1.1L from the
muzzle toward the target. Native weapon-deflect precedes damage/burn/push;
blocked targets still consume a chain hit/deduction.

### 8.2 BF2 emitter lightning and its authored effect

`com_sfx_lightning` is a top-level `LightningBolt`, not a ParticleEmitter.
INGAME's record authors Texture=com_sfx_lightning, Width.5, FadeTime.6,
BreakDistance1, TextureSize3.5, SpreadFactor.8, InterpolationSpeed5,
MaxBranches3, NumChildren3, BranchFactor.15, ChildWidth.4,
ChildTextureSize2, ChildSpreadFactor1, Color[130,170,255,250] and
ChildColor[140,180,255,100]. The bake preserves missing fields rather than
substituting particle defaults, and includes its texture dependency.

FUN00458a50,518590–518658, defaults missing BranchSpreadFactor.8,
BranchLength4, ChildBreakDistance1.5, ChildTextureSize3, ChildWidth.3,
ChildSpreadFactor1, TextureSize5, BreakDistance2, Width2, FadeTime1,
SpreadFactor1, InterpolationSpeed10, BranchFactor.3, MaxBranches5 and
NumChildren3; MaxBranches is capped10 after parsing. Both colors defaultwhite.

Emitter class+c4/hash38ffe98e multiplies shot charge to **bolt strength**
(FUN0045a5b0 sets livebolt+104). It does not set lifetime. Lifetime starts
from FadeTime (00459270);004594f0 decrements it and advances a pair of jitter
seeds when phase>1. FUN00459898 applies width multiplier.7*strength+.3 and
alpha multiplier min(1,2*remaining/FadeTime)*(.5*strength+.5), then renders
max(1,int(NumChildren*strength)) child bolts when NumChildren>0.

FUN00459c50 uses max(3,int(length/BreakDistance)) segments and skips counts
above100. Each interior point gets a seeded 3D offset projected perpendicular
to the bolt, interpolating two offset streams by phase. Width tapers by
initialWidth/(segmentCount+1) per interior point. Both endpoints have alpha0;
interior points retain the computed alpha. A sampled branch can extend along
normalize(jitter+axis*BranchSpreadFactor), up to MaxBranches, with length
BranchLength; a child branch has no further branch children. Texture coordinate
increments are segmentLength/TextureSize. The port uses the ordinary shared
particle renderer for its view-facing strips; exact GS vertex joins and the
session RNG sequence require separate presentation evidence.

## 9. The disc: fielded primaries that are not bolts

BF1 (UAUD1, `analysis/unit-audit-bf1.md` §3.1): missile = the torpedo launchers (ALL vanguard x3, IMP shock trooper
x2, REP ARC trooper, CIS assault droid; the Wookiee rocketeer local), + the SBD's wrist rocket (ch1) and the
Geonosian blunderbuss (a missile with TurnRate 0, LifeSpan 0.85); shell = the IMP pilots' and CIS pilot droid's mortar
`_ord1`; emitter = the clone pilot's arc caster; bullet = the jet trooper's launcher. BF2 (`census2_all.md`): missile
= 9 rocket / torpedo launchers, the EMP launcher (x3), the bulldog (x2); bullet = shotguns, award shotguns and
pistols, the chaingun, Jango's pistol; emitter = the incinerator, the Imperial and Jawa arc casters, two hero
flamethrowers.

## 9b. The fat ray and the beam [V] (2026-10-30)

- `fatray` (the sonic blaster's secondary, the geonosian turret): class ctor `FUN_003183f0` (B2:295295, the bullet class
  plus seven floats), setter `FUN_00318520` (B2:295351): MaxDamage 0x47d0e794 (+0x11c, ctor 20), 0xfe544f96 (min, +0x120,
  10), 0xaf922a6b (deduction, +0x124, 5), 0xaee05d99 / 0x3ed11ac3 / 0x58a7da24 (push max / min / deduction, 15 / 5 / 3),
  RayRadius 0xa4c53efd (+0x134, 3). Instance `FUN_00317e10` (B2:295036): damage = lerp(min, max, charge), push alike.
  Sweep `FUN_00317f20` (B2:295077) replaces the bullet sweep: bodies of mask 0x1022 (no vehicles) on the tick's
  segment, owner skipped, at the soldier's target point with 0 < t < L and distance d < radius; f = 1 − d/radius;
  damage x f with every scale 1.0, then damage = max(damage − deduction x f, min); it returns 1 (the round flies on).
- `beam` (the BF2 award sniper rifles, the vehicle beams): class `FUN_003126c0` (B2:291369, 0xe0 bytes), setter
  `FUN_00312838` (B2:291429): Range 0xfadc0cd2 (+0xbc, ctor 150), LaserWidth / LaserTexture / LaserGlowColor, the pierce
  bit 0x604e2441 (+0xd8), ImpactEffect 0x11727a92 (+0xdc); the bullet ImpactEffect* keys are dead on it. Instance
  `FUN_00311b50` (B2:290798) holds the owner's nearest fire point. Update `FUN_00311d00` (B2:290892-291107), every frame
  of its LifeSpan: origin and direction again from the fire point, a ray of Range; an entity struck takes the class's
  damage desc x dt / LifeSpan and spawns the +0xdc effect; without the pierce bit the first hit stops it, with it a
  soldier-type hit (0x80 / 0x10 / 4) still does; the drawn ribbon is the reached length + 1 about its middle.
- The port: `Sim::sweep_fat_rays` (`sim/fatray.rs`); `Sim::beams` (`sim/beam.rs`) casts one full-range round a tick
  through the bolt sweep, with damage x TICK_DT / LifeSpan. Deviations: a pierce beam stops at the first vehicle too;
  a seat's beam keeps its spawn ray; the per-hit impact effect has a network-looking gate (DAT_00600548 / 0x549 /
  0x560 / 0x6d85ad) that is taken as open.
- B1's bullet render `FUN_00277130` (B1:197885) draws the class model (GeometryName, the bullet setter's +0x84) along the
  velocity for any bullet-family round: the jet trooper's rocket is drawn as `com_weap_missile`.

## 10. The port today, and the build

The port (main 4286042) builds every channel-0 ordnance as a straight bolt (`game/src/roster.rs` `weapon_from_db`),
one predictive ray at spawn against terrain, world AND units (`core/src/projectile.rs`), the impact explosion (RKEX),
no ExplosionExpire for a bolt, and the `sticky` path (GREN) for channel-1 grenades only. Gaps, by what a player sees:
1. **Missiles** (both games): the update law of §4.3 (ramp, turn, waver; BF2 speed ramp and bot aim error), the
   full-mask per-tick sweep, homing on a locked target's point (§4.4), the dwell lock for launchers (§5: human by the
   magnet / HUD lock, bot by its combat target, one lock per shot, bots do not wait), no gravity. Card UORD (build).
2. **The bolt family's rays** (every rifle): predictive ray against terrain + buildings only, a per-tick sweep against
   units / hulls / water / foliage / shields — dodging and stepping in both work on the PS2. A behaviour change for
   every gun; its own card, UORDS, value 4.
3. **Shells**: gravity, no predictive ray, explode on contact, ExplosionExpire in the air; the BF1 mortar's
   charge-stage swap (tap = shell, hold = sticky) on channel 0. With UORD (same per-tick ordnance path).
4. **Emitters**: the instant cone-and-chain query per shot (BF1 rules; BF2 + hit-all, burn, team filters). Card
   UORDE, value 3.
5. Bullet: nothing beyond 2 and ExplosionExpire; the BF2 shotgun's tracers are visual.

## 11. Checks run (output in `lead_tmp/uord/checks/`)
See `checks/README.md` for the commands; receipts: [receipt:c7aceac5] (0x73e5a988 / LockedOnSound unread elsewhere,
H1), [receipt:caf31d73] (no flare / decoy, H1), [receipt:b28f880b] and [receipt:ef4124ba] (Rebound / Friction / Gravity /
ExpireEffect / ShotElevate readers, H3), and the lead's own: [receipt:11d0e4d8] (Gravity's readers in every form:
the item setters, an armed-building class, the shell setters and a B2 chunk parser `FUN_00307a80` - no missile,
bullet or bolt setter), [receipt:859265a3] (the launcher lock getters have no caller but their vtable words; refs.py
in `checks/refs_lockgetter.txt`). The BF1 AI fire functions read no 0x168 and call no weapon-instance vt+0x34
(`checks/ai_fire_no_lock_b1.txt`; the one `+0x34` call in `FUN_0023c9c8` is on the weapon CLASS vtable). Stores to
the predictive flag: `checks/stores_88_b1.txt`, `stores_c0_b2.txt`. ELF words: `checks/words.txt` (0.2 impact offset
both games, 2.5 / 0.1 emitter widening, 9000.0 = `lui 0x460c` + `ori 0xa000` at B2 0x31ef2c, the vtables).

### 11.1 The workflow verifier (`uord-re-verify`, 6 agents, claims and citations only)
42 claims: 32 in four batches of 8 (one verifier each), 10 absence / twin claims to two separate verifiers (A, B).
52 verdicts: 44 SUPPORTED, 8 PARTLY, 0 NOT SUPPORTED (`verify/verdicts.json`). Fixed in this doc:
- C16 (the mortar's stage step is the shared charge-stage function, can step several stages) - §6.
- C17 (MinSpeed's fall-through also writes Acceleration) - already §4.1.
- C22 (the exit-fire clear needs class+0x254 as well; LockTime ≤ 0 falls back to the base charge test; the clock limit
  falls back to class+0x1e0) - §5.
- A3, both readers (the repair weapon's own vt+0x34 `FUN_002927d8`) - §5.
- A5, reader A only (B2 `FUN_0022dc20` is an armed-building setter, not an item; the dead-key conclusion stands per
  both) - wording.
- T3, both readers (BF2's dwell picks its own candidate and resets with no target; BF1 can dwell on nothing) - §5.
No claim was supported by one reader and refuted by the other; the second reader (B) overturned nothing reader A
supported.

## 12. What stayed [H]
- The reader of BF2's occupant warning fields (c+0x10c..+0x118) — the HUD's missileLock events likely.
- The BF2 HUD lock acquisition's pick rule and button (`FUN_0017a908`).
- The live share of guided BOT missiles (needs a battle with vehicles in a launcher bot's sight).
- The double explosion of a predictive-hit bullet with both explosion keys; the BF2 pellet falloff steps.
- Why the BF1 player missile's +0x104 reads 0 every other update.
- The emitter's open keys (13 BF2 hashes unnamed), which weapons answer the deflect query, BF2 gather layer 0x1000.
- The name of 0x73e5a988 (unauthored; FNV "ScaleTime" only).
- The beam's reach for the AI's range gate: the port keeps Velocity x LifeSpan; no beam range getter was found.
- BF2's waver live (sign, phase order): the capture's BF2 classes all have WaverTurn 0; the EMP launcher's ordnance
  (WaverRate 0.2, WaverTurn 0.015 on the disc) would exercise it (AUDIT).
- The BF1 crouch / prone target heights (0.6 / 0.2) and another BF2 class offset: the homing rows hold standing
  targets only (AUDIT).

## Issue #90 follow-up verification (2026-11-01)

[V] Reopened BF2 `FUN_00311d00` by address (historical corpus line numbers
have shifted). Every beam update invokes the victim's `+0xdc` deflection slot
before applying damage scaled by `dt / LifeSpan`. An accepted block exits
that update's ray loop; there is no once-per-held-beam block-energy latch.
The port therefore keeps one block payment per contact tick. The fixture uses
2.5 energy/contact and three contacts, leaving 92.5 of 100; those are fixture
values, not a newly inferred universal weapon cost.

[V] `FUN_00314568` builds a fresh full-charge reflection packet with no target
and invokes the ordnance class factory (`+0x10`). Reflected missiles must
recreate their missile flight state (speed ramp/waver), without inheriting the
incoming lock. The port restores that state without creating another held beam.

[V] The emitter query `FUN_00315f78` and BF1 `FUN_00278208` use `ConeLength`;
a projectile's velocity-times-lifetime is not a cone's reach. AI firing and
weapon reach checks now share `WeaponClass::shot_range`.

[D] Pure visual secondary rounds bypass dynamic contact only with zero direct
damage **and no impact explosion**. A zero-direct-damage explosive still
collides. This fixes hit accounting, bot hit notification and saber billing.

[D] The BF2 REP class database's `rep_weap_walk_oneman_atst_cannon_ord`
inherits the mortar and names `com_weap_inf_grenadethermal_ord` as geometry.
The walker tracer path previously submitted a second draw despite the ordinary
bolt path preferring geometry. Both paths now apply that same geometry priority.
Auto-turret launcher locks use the turret's unique ID and live centre; both the
held lock and missile flight state hash that ID with a distinct target tag.

## Issue #87 — saber, burn and area shield rules (2026-11-01)

[V] The following functions were reopened by address, rather than relying on
historical line numbers. BF2 `FUN_00315f78` calls the target's `+0xdc` deflection
slot before cone damage, burn and push. Saber blocking of emitters is native.
BF1's older emitter lacks that hook; the port retains its existing BF2 block
rule on both games' maps, consistent with the unified gameplay policy.

[V] BF2 `FUN_00317f20` calls the same slot for every fat-ray victim. Acceptance
skips that victim's damage, **not** the remainder of the ray: the per-victim
falloff deduction runs on either branch and traversal continues. The port now
invokes deflection at the victim and retains the original ray. Making the saber
terminate the entire fat ray would be a deviation, not a fidelity fix.

[V] Droidekas do not burn. Before calling `FUN_00236e28`, `FUN_00315f78`
requires the exact entity type returned at `+0xc` to equal `DAT_00689828`
(soldier, returned by `FUN_00256398`). The droideka's main vtable is `0x61bc50`;
ELF word `0x61bc5c` is `0x001ffa38`, whose instructions return `DAT_00689300`,
not the soldier ID. Existing no-burn handling is retained. This is not inferred
from the visual absence of fire.

[V] Emitters and fat rays call `FUN_001e0900` with flags `-1`; the native health
routine `FUN_001dfb38` applies bit 0's shield phase, then bit 1's health phase.
Friendly-fire and attacker scaling precede shield subtraction; positive damage
× ShieldScale reduces the active shield. Overflow divided by ShieldScale is
removed from incoming damage before the health-type scale. ShieldScale=0 skips
the shield phase; it does not discard health damage. `FUN_0032d198`, the
area-effect/Choke damage path, likewise calls the damage slot with flags `-1`
and all scales 1. The port routes cone, fat-ray and Force area damage through
the existing shield-lending helper, also used by explosions. Physical bolt and
blade body contacts keep their separate collision/shield rules.

Recheck commands: `python3 scripts/re_check.py word b2 0x61bc5c 1 --u32`;
`python3 scripts/tools/eedis2.py bf2 1ffa38 1ffa58`. Test coverage includes
raised/lowered/depleted shields, a zero shield scale, burning exclusion, emitter
and choke damage, and a blocked fat ray reaching a second body.
