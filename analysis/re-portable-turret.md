# OPUS RE (2026-10-28) - [O] UNAUDITED — The BF2 sniper's portable auto-turret: the dispenser, the `portableturret` entity, its gun and its AI (card ATUR)

Corpora: B2 = `analysis/SWBFRNT2.ELF-decomp.c` (SLUS-21240); line numbers are of that file. Tags: [V] read at the cited
lines, [M] measured on the PS2 (PCSX2, live RAM), [D] derived, [H] hypothesis, [O] load-bearing for the port. Anything
not [V]/[M] is PROVISIONAL in code. Disc values: the baked class chains (`engine/assets_baked/classes/bf2_*.ron`), read
by name and by FNV-1a hash (lower-case bytes). The AI half leans on `analysis/re-turret-ai.md` §2, §3 and §6.
**No live PCSX2 capture was made for this note.** Nothing here is [M]. BF1 has no portable turret.

## 0. What the PS2 does, in one page [O]

| question | BF2 | tag |
|---|---|---|
| who carries it | the four `<side>_inf_default_sniper` classes and every class that inherits their slot: 14 soldier classes counting variants (all ×5, cis ×2, imp ×3, rep ×4); slot 4, channel 1, 1 clip | V (disc census, `game/tests/suite/auto_turrets.rs`) |
| the weapon | `com_weap_inf_autoturret_dispenser`, a `dispenser`: `OrdnanceName` names the turret entity; `MaxItems` 1, `ShotDelay` 1.5, `InitialSalvoDelay` 0.5, `ReloadTime` 2.0, `OffhandWeapon` 1 | V |
| the throw | aim × (`MinStrength` 0.3 + (`MaxStrength` 0.4 − 0.3) × charge) × `Velocity` (not authored: 10) + the owner's velocity, so 3–4 m/s; team = the owner's | V |
| the limit | past `MaxItems` the oldest turret of that weapon is killed through its damage interface and dropped; a full pool kills the oldest of the type and retries | V |
| the entity | `com_bldg_inf_autoturret`, class `portableturret` (labelled `armedbuildingdynamic`): `MaxHealth` 50, `Lifetime` 60, no `HealthType` (so Building), model `com_weap_inf_dropturret` | V |
| flight | half-step gravity (`Gravity` 1 × 9.8), lands on a contact whose normal has y > 0.7 (frame up = n, velocity 0), otherwise bounces by `Friction` 0.5 / `Rebound` 0.3 | V |
| life | ages while `Lifetime` > 0, dies past it; dies below y = −500 | V |
| the gun | `<side>_weap_inf_autoturret` (cannon): `RoundsPerClip` 0 (unlimited), `MinRange` 0 / `OptimalRange` 15 / `MaxRange` 40, `ShotDelay` 1.7, `SalvoCount` 4, `SalvoDelay` .15–.3; bolt 100 m/s, 0.8 s, `MaxDamage` 100, vehicle / building × 0.1 | V |
| its aim | `PilotType self` → a `UnitStationaryController` running the wait agent: tracks to about 100 m, fights a target inside MinRange..MaxRange that the gun's `AITarget` mask allows, keeps the fight to MaxRange + 10, fires inside MaxRange within 22.5°; idle look base yaw ± 30°, redrawn every 18 thinks; turns by the hover stick law | V (`re-turret-ai.md` §2-3, §6) |
| its death | `ExplosionName` `<side>_bldg_inf_autoturret_exp`: 50 damage over 4 m, the landmine effect | V (disc), H (on expiry / replacement too) |
| where it hangs | the entity's origin is the landing point; the model's sphere (radius 0.25) sits at its `boneroot`, 1.056 m up the frame, so it hovers about 0.8 m clear of the ground | V (skel), D (origin = contact point) |
| its look | `com_prop_dropturret_rep` (128×128, only in the SIDE LVLs; every played side's `OverrideTexture` names this same sheet); a lit shell and an UNLIT band whose UVs scroll at −200/255 and −1 repeat per second | V (§7) |
| bots | a bot presses it when its target is a soldier on foot in a cluster of his own side (more than 3 within 9 m, none hostile) and the dispenser is ready; no range test, no live-turret test; 1 round per life | V (§6.1) |

## 1. Class labels [V]

| label | B2 |
|---|---|
| entity `portableturret` class setter | `FUN_0022dc20` (B2:144089) |
| class defaults | `FUN_0022d980` (B2:143991) |
| class copy | `FUN_0022da10` (B2:144012) |
| factory (0x720-byte pool, 0 when full) | `FUN_0022db90` (B2:144066) |
| instance constructor | `FUN_0022ce08` (B2:143353) |
| update | `FUN_0022d1e0` (B2:143580) |
| contact | `FUN_0022d5b0` (B2:143760) |
| the dispenser weapon class family | `FUN_00336858` (B2:316613); the dispense at about B2:315990-316790 |

## 2. The class keys [V]

`FUN_0022dc20` (B2:144089) reads, by hash:

| key | hash | offset | default (`FUN_0022d980`) |
|---|---|---|---|
| `Friction` | 0xa51be2bb | +0x6b4 | 0.5 |
| `Lifetime` | 0x5212abee | +0x6a8 | FLT_MAX |
| `Rebound` | 0x678de338 | +0x6b0 | 0.3 |
| `Gravity` | — | +0x6ac | 1.0 |
| `CollisionSound` / `CollisionOtherSound` / `CollisionWaterSound` | 0x2297e47d, … | +0x690 / +0x698 / +0x6a0 | none |

Everything else (`MaxHealth`, the `TURRET1` section, `ExplosionName`, `OverrideTexture` 0x09db74ac) goes to the
armed-building base. The disc's TURRET1: `PilotType self`, `TurretNodeName aimer_y`, `PitchLimits -35 35`, `YawLimits
-180 180`, `MaxTurnSpeed 3.8`, `MaxPitchSpeed 3.8`, `AimerNodeName aimer_x`, `FirePointName hp_fire`, `WeaponName
<side>_weap_inf_autoturret`, `TurretAmbientSound remotedroid_engine_lp`, `TurretActivateSound vehicle_equip`.

## 3. The update [V]

`FUN_0022d1e0` (B2:143580):

- `age (+0x6f4) += dt` while the class's `Lifetime` > 0 (B2:143601-143603).
- `Lifetime < age` → kill through the damage interface (`(+0x22c)->vtbl[3](+0x170)`, B2:143622-143636).
- `y (+0x154) < −500` → the same kill.
- [H] A third kill: with flag +0x228 bit 3 clear and both class words +0x384 and +0x4d4 zero, the same branch kills
  without the lifetime test. The flag was not identified; the port does not model it.
- The "just landed" byte +0x715 is cleared into the landed byte +0x714 and the frame refreshed (B2:143645-143651).
- Not landed: `vel.y += Gravity × dt × −4.9; pos += vel × dt; vel.y += the same` (B2:143653-), the half-step fall the
  port's mines use.

## 4. The contact [V]

`FUN_0022d5b0` (B2:143760), only while not landed (B2:143805):

- The collision sound by contact type (0x200 water, 0x100 the "other" sound, else the plain one).
- Contact types 2 and 8 are skipped (no bounce, no landing) [D: soldiers and ordnance].
- `vn = vel · n < 0` and `n.y > 0.7` (B2:143871) → land: frame up = n, velocity 0, the +0x715 byte set.
- Otherwise the bounce: `vt = vel − n vn`, `f = min(−vn × Friction × (Rebound + 1) / |vt|, 1)`,
  `vel = vt (1 − f) − n vn Rebound` — the formula of the mine's non-sticking branch.

## 5. The dispense [V]

The BF2 dispenser (the `FUN_00336858` family, B2:~315990-316790): when the class's `MaxItems` (+0x260) > 0 and the
weapon's list is full, the oldest item is killed through its damage interface and dropped; a full pool (`FUN_0022db90`
returns 0) kills the oldest of the type and retries. The item gets `aim × speed + owner velocity`, the owner's team, and
the owner at +0x710. An item whose controller IsA the stationary controller (DAT_00698178) gets no order from the
dispenser (`re-turret-ai.md` C12): its own controller runs it.

## 6. The AI [V via re-turret-ai.md]

`PilotType self` gives the turret its own `UnitStationaryController`; the every-15th-step pass hands it order 10 (the
wait agent). `re-turret-ai.md` §3 and §6 apply as written: view ≈ 100 m; a target is taken when the gun's `AITarget`
mask (Enemy / Person / Droid / Vehicle / Building) allows it and it is within MinRange..MaxRange; a held fight lasts to
MaxRange + 10; the trigger is pulled inside MaxRange with the aim within 22.5°. With no target the look point is the
base yaw ± 30°, redrawn every 18 thinks. The turn is the hover stick law `u|u|`: yaw on the sine of the error (±1 when
the target is behind) with k = 4.5 − clamp(`MaxTurnSpeed`, 1, 3) = 1.5, pitch with k = 5.0 − clamp(`MaxPitchSpeed`, 1,
3) = 2.0, each times its rate. The cannon's trigger is a burst trigger.

### 6.1 A bot's use of the dispenser [V]

The soldier think runs three item rules after its target pass (`FUN_002b8160`, `FUN_002b8258`, `FUN_002b8400`;
B2:226983-226985). `FUN_002b8400` (B2:227676-227728):

- the unit is a soldier (IsA `DAT_00689828`) and has a target (`FUN_002b9fe0`), itself a soldier;
- the target's controller passes `FUN_002aec90` (B2:221264): its count +0x8c > 3 and its hostile count +0x88 = 0, the
  cluster test the grenade choice uses (`re-soldier-ai.md` §4.3, 7.2);
- then for every weapon of the bot: one that is not empty (`FUN_00327820`) or busy (`FUN_002b8760` / `FUN_002b8738`),
  IsA `WeaponDispenserClass` (`DAT_00699f20`), whose item IsA `EntityBuildingArmedClass` (`DAT_00688ed0`), and whose AI
  mask +0x124 holds bit 1 (Person) is selected and fired (`FUN_002bbed0`, `FUN_002bbb60(.., 1, 0)`).

There is no range test and no test for a turret of his that still stands: the rate is the ammo (`WeaponAmmo` 1, so one
throw per life), the reload (`ReloadTime` 2.0) and `MaxItems` (a second throw replaces the first). The mask's default
is 0xffffff9f (weapon class constructor, B2:307279-307280), and the disc's dispensers author no `AITarget*` key, so
bit 1 holds; their `TargetPerson 0` writes the other word (+0x120). The weapon-choice suitability (`FUN_002aaba0`,
B2:218296) never picks a dispenser: its rate +0x88 is 0, because the dispenser's setter `FUN_00336a50` stores
`OrdnanceName` at +0x240 and returns (B2:316745-316754).

## 7. The model [V]

`com_weap_inf_dropturret` is in INGAME.LVL; its sheet `com_prop_dropturret_rep` (128×128, 8 bpp) is in SIDE/ALL, CIS,
IMP, REP and TUR, not in INGAME. Each played side's turret class names that same sheet in `OverrideTexture`, so the
override changes nothing for this model; TUR's names `com_prop_dropturret_all`.

The model is bone-space. Its skeleton: `dummyroot` → `boneroot` (0, 1.0564066, 0) → `aimer_y` (0, 0, 0) → `aimer_x`
(0, 0, 0.23444428) → `hp_fire` (0, 0, 0.07409216). At the bind pose the shell is a sphere of radius 0.25 about
`boneroot` (y 0.806..1.32). The entity is placed by its origin, which the landing puts on the contact point (§4), so the
sphere hangs about 0.8 m above the ground. The aimer and the fire point face +Z, the model's front. Two segments:

| segment | MTRL flags | RTYP | UV bytes (MTRL 16, 17) | draws |
|---|---|---|---|---|
| shell | 0x00010001 | 0 | 0, 0 | lit |
| band | 0x01010010 | 3 | 200, 255 | unlit (no 0x01), UV scroll (0x1000000) |

The UV animation (`re-material-flags.md` §BF2): `FUN_0058e730` (B2:710803) copies the two bytes into the segment's
animation record when 0x1000000 is set. Each draw, `FUN_0058e930` (B2:710887-710990) offsets the UVs by
`u = −b0 · t / 255`, `v = −b1 · t / 255`, with t the seconds since the scene's start (`(Count − start) >> 15` ×
`DAT_00661318` = 1/9000 s, `a5-tickrate.md`), each reduced into (−1, 1). So the band scrolls 0.784 repeats per second in
U and 1 in V. The LOWD model's band has bytes 6, 0. The HUD shows the same model (`hudtransforms` `namemesh
com_bldg_inf_autoturret com_weap_inf_dropturret`), so its band scrolls too.

## 8. The port [O]

- `core/src/autoturret.rs`: `TurretBody` (the keys of §2 and TURRET1), `TurretClass`, `Turret`, `TurretEnd`.
- `core/src/sim/turrets.rs`: the throw (`throw_from`, shared with mines, droids and packs), `oldest_beyond` (shared with
  mines), the fall (`mine::fall`, `mine::rebound`, `sweep_item`), the think every 0.5 s, the turn, the fire through the
  gun's own `WeaponState`, `free_shots` (shared with the droid's blaster), the end and the explosion.
- Damage: `Target::Turret`, a 0.25 m sphere for bolts (erratum 2026-10-29: was written 0.3; the code and §8 below use `RADIUS` 0.25) that skips the owner's own; blasts through the shared items pass;
  both by the `HealthType` scale (Building).
- Bots: §6.1 in `ai/abilities.rs`, the cluster through the grenade's `group_counts` / `is_group` (BF2); the
  dispenser is excluded from the weapon choice (`WeaponClass::dispenses`).
- The model: `bake weapon INGAME.LVL,SIDE/REP.LVL com_weap_inf_dropturret` bakes it posed at its bind pose with its
  sheet; the GLB material carries the band's `uv_scroll`. The world draw gives that part its own scrolling texture and
  turns the model by yaw + π (its front is +Z); the HUD scrolls the part's UVs by its clock on a repeating sampler.
- The turret's `pos` is the origin; the centre is `pos + up × 1.056`, the hit sphere 0.25 m, the muzzle 0.234 m along
  the yaw and 0.074 m along the aim from the centre (`aimer_x`, `hp_fire`).

## 9. What stays open [H]

- [H] Expiry and replacement explode through `ExplosionName` (the kill goes through the damage interface, whose death
  path explodes armed buildings; the explosion on that path was not traced to the call).
- [H] The turret outlives its owner; nothing in §3 tests the owner.
- Repair: settled in section 10.
- OURS: the think interval (0.5 s), holding the trigger while the gun's `ShotDelay` / salvo pace it, the aim as
  world-frame yaw / pitch. The hit and soldier-contact sphere is the authored prim (§10).
- [D] The landing puts the origin on the contact point. The authored primitive was subsequently read (§10).
- [H] Whether the cluster count holds the target itself (the port's `is_group`: more than 4 counting him).
- Not modelled: the aimer pitch pose (the model is drawn rigid), the band's unlit draw (weapon parts are drawn with the
  character light), `OverrideTexture` as a general mechanism, the ambient / activate / collision sounds, the third kill
  of §3 (now section 10). Soldier collision and damage effects are implemented in §11.

## 10. Destroying it (the maintainer, 2026-10-30) [O]

"The auto turrets: Can those not be destroyed yet? They don't have health and attacking them doesn't do anything.
They should have health and be attackable like the original games had."

**Which entities.** In a conquest round (the port's only mode) the only unmanned auto turret is this one, the BF2
sniper's dropped `<side>_bldg_inf_autoturret` (base `com_bldg_inf_autoturret`, `portableturret`), on every BF2 map whose
missions field a sniper. Census of the port's picked conquest layers over all baked maps (`mode_layer_index`'s rule): no
`PilotType self` instance. The placed ones stay in other layers: HOT1 `hoth_objective` + `hoth_ctf` / `hoth_hunt`
(`tur_bldg_chaingun_tripod`, `_roof`), KAM1 / MUS1 `*_campaign` (roof gun), FEL1 / NAB2 `*_campaign`, MYG1 `myg1_cp-assault`
(`tur_bldg_recoilless_*_auto`), UTA1 `uta1_campaign` (card VSELF: they never fire; they already are armed buildings with
health). BF1 has no auto turret but the fambaa's two (card FAMB; NAB1_H only, not in any baked manifest instance). Remaining
armed classes in conquest layers are manned.

**What was wrong: not the pool.** `MaxHealth` 50, bolts and blasts by the Building scale already ran (`Target::Turret`,
`damage_item`). Measured on BF2 Naboo, reticle dead on the centre, parent commit:

| | standing 6-38 m | crouched 6-30 m |
|---|---|---|
| rifle | destroyed in 7 bolts | 0-40 of 50 HP off in a clip's 4 s: bolts pass under |
| rocket | destroyed; missed at 38 m | missed at every range to 38 m |

- The aim ray (`Sim::aim_hit`) ignored it, so the bolt flew from the fire point to the ground behind: 0.5 m under the
  ray at 15 m crouched against a 0.25 m sphere.
- The reticle's pick ignored it: no name, no health bar, no colour, no hit flash. Hence "they don't have health".
- Bots never took one ("not modelled" above).

**The PS2.** It is an armed building (`re-vehicles.md` line 545) and the disc's model has one prim,
`p_-so_sphere` r 0.25 on `boneroot`, mask 17 (soldier + ordnance): the port's `RADIUS` is that prim, not ours.
- The acquisition takes a building, and the readout shows its name and health (`re-target-hud.md` headline, B2:410093-410122);
  the class has no `HideHealthBar`. `TargetBuilding 0` on the rifle and launcher keeps the magnet off it, so the pull
  is unchanged: only `com_weap_inf_autoturret`-like weapons with the bit pull. The name is Locl 0x664ffe37 "Auto Turret" [D:438921].
- Hit call: `FUN_001e0580` calls `FUN_0035fd50` for any victim of enemy relation (B2:91673).
- Death: dead with no destroyed (+0x384) and no unbuilt (+0x4d4) model, the update kills it through the damage interface at
  once (`FUN_0022d1e0` B2:144295-144302; this is the "third kill" of §3, the flag being the alive bit,
  `re-build-zones.md` 1): the class authors neither, so it is **gone, with its explosion; no wreck and no rebuild**. The
  maintainer's "only fixed by a fusion cutter" rule has nothing to act on once it is down.
- Repair of a standing one: the cutter's amount is by the target's health type with no class test
  (`FUN_00346fa0` B2:339872): Building's `BuildingHealth`, friend's only (the cutter's team mask). [V]; the port now does.
- Credit: BF2's point events are soldier kills and posts (`event_feed::points`, `re-event-messages.md` table), and no line
  names a destroyed building. **No points, no stat for destroying one** (the turret's own kills already credit its owner).
- Bots: `FUN_002e3aa0` asks `FUN_002a7c30` (alive, finite health, hostile), view × 10 for an armed building
  (`FUN_002e3910`), radius by the longest weapon + 5 × speed (`FUN_002a9d90`), then the weapon choice
  (`FUN_002ac8f8` -> `FUN_002abee0` -> `FUN_002ab590` -> `FUN_002aaba0`): the mask is `AITarget*`. Disc: rifle, pistol, grenade
  mask 0x7 (no Building): **a rifleman walks by; a rocketeer (`AITargetBuilding 1`) takes it.**

**The port** (`[ours]` called out):
- `AimTarget::Turret(id)`: the pick, HUD name and health, the magnet where the weapon's bits allow.
- `Sim::aim_hit` stops on a live turret or droid of another owner.
- `Event::TurretHit` carries the plain hit flash.
- `Foe::Turret(id)`, `AimRequest::Turret`: the scan, weapon choice and fire, as `Foe::Droid`. `[ours]`: the aim error's
  size term is the droid's 0.17; a weak-weapon rule of 22.1 steps 5-6 is left out (no held weapon reaches it).
- The cutter mends a standing friend's turret. Bot engineers now include a damaged landed portable turret in their nearby aid search (20 m, one engineer per target), yielding to combat as they do for vehicle/droid aid (issue #91).
- **Same gap, same fix:** a remote droid's sphere is on the aim ray too. Its reticle readout stays unported (a fast
  flyer would change the magnet's feel; follow-up).
- Not ported: soldiers colliding with the sphere (mask `s`), the class's damage effects (sparks under 50, smoke under 30).

## 11. Portable collision, effects and engineer aid (2026-11-01)

The soldier wall pass now includes the shell's authored sphere (§10), with the
same horizontal resolution convention as world walls. It sweeps in half-capsule-radius
steps, blocks either team while landed and alive, and removes the into-contact velocity.
A prone soldier fits under the shell. The flying item's own contact still skips soldier
and ordnance types: re-read BF2 `FUN_0022d5b0` (current decomp lines 144445 onward),
which excludes contact types 2 and 8 before the bounce/land branch.

The class's `DamageStartPercent`, `DamageStopPercent` and `DamageEffect` slots now
reach the visual effect pass, including portable-turret properties authored after
`TURRET1`. BF2 `com_bldg_inf_autoturret`: `com_sfx_vehiclespark` at 50–30% and
`com_sfx_smokeplume` at 30–0%. The named smoke attach point `hp_smoke_1` is absent
from the original model's five-node skeleton (§7); both effects therefore use the
entity frame. That fallback is provisional, not a new live PS2 observation.
Each health-band emitter is held once per turret identity, follows its frame,
and stops emitting on repair, death, expiry or replacement; emitted particles finish.

Engineer aid uses the existing repair amount by health type: BF2 `FUN_00346fa0`
lines 339869–339873 reads the target's low three type bits and the health-amount table.
The search/priority remains the port's nearby aid policy (20 m, no known enemy),
not a claim of matching the PS2's 75 m search. A single engineer claims each portable
identity; full health, death and expiry retire the claim. No wreck or rebuild is added.
