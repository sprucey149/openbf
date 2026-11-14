# Ground vehicle and anti-vehicle normalization

BF1 ground battles take BF2’s health and weapon numbers for equivalent vehicles
and turrets. A fighter tank has 8,000 HP in both games. Its light cannon takes
two clean hits to kill a 400-HP trooper at full damage, up from one on the previous
BF1 scale. A rocket that reaches full damage takes 1,500 HP off a hull, up from 600.
A speeder bike has 80 HP, down from 600. This is a deliberate port rule.
Measure the change with awards off using the frozen proxy, both factions, and
the Conquest bot census. Controller feel and original-game play remain separate
checks.

The stock BF1 Imperial and CIS tap shells use the original 3 s shell default
for their missing lifetime key. The BF2 taps author 8 s. This is a retained
motion difference alongside the shared damage numbers. The focused expiry
witness is in [ordnance kinds §6](re-ordnance-kinds.md#6-the-shell-and-the-mortar-launcher-vo-h3-3-4).

## Rules in the loader

1. **Shared classes.** The BF1 class takes its BF2 twin’s MaxHealth. The existing
   ground weak parts and their factors still come from the class and hull
   rules. Matching guns take the BF2 damage, damage scales, fall-off, rate,
   salvo, heat, magazine and reload. Some rows also copy turn rate, charge
   points or beam penetration. Fighter-tank gunner beams and the spider
   walker beam now pass through infantry and vehicles on ground rounds.
2. **BF1-only hulls.** MTT and hovernaut use the AT-TE health ratio, 40,000/15,000.
   The Gian speeder uses the combat speeder ratio, 8,000/5,400. The skiff uses
   the tank ratio, 8,000/6,600. The landspeeder uses the bike ratio, 80/600.
   The Bespin AAA turret uses the gun turret ratio, 800/300. Their own guns
   keep the soldier pass’s ×4/3 and take ×8,000/6,600 against vehicles. The
   skiff mounts the common gun turret’s weapon and takes that weapon’s shared
   numbers. The MTT uses a dedicated copy of its MAF cannon so the flyer’s
   numbers follow its own rule. Measure kills per minute in and against these
   vehicles before and after the pass.
3. **Infantry ordnance.** The BF1 torpedo adopts BF2’s 500–1,500 age-based
   ramp. Thermal detonators, land mines and remote charges take their BF2
   damage and scales. The Imperial and CIS pilots’ tap mortar shell does 400 on contact and a
   separate blast of up to 400. The charged sticky stage carries a 100
   contact descriptor in its class data. The sticky simulation applies
   the blast when it detonates. Listed light arms use 0.1 vehicle
   and building scales on their BF1 base damage. BF1-only crack and haywire
   grenades take the tank ratio on their vehicle scale, including cases
   where the existing scale is zero. The BF1 launcher’s existing lock rule
   and splash radii were established before the vehicle pass.
4. **Turrets and borrowed weapons.** The laser turret supplies the common
   gun, Yavin tower and sail barge gun numbers. The common gun’s impact
   does zero damage. The turret and barge retain their authored death blast.
   The Hoth dish uses BF2’s four authored charge points. As in BF2, the
   3,000-damage point is at charge 2.0, beyond the player’s charge cap of
   1.0. A fully charged shot interpolates to 1,800 direct damage. The BF1
   Geonosis turret supplied Damage 400 where the ray reads MaxDamage and used the default
   20. It now reads 400 at full charge. The ray’s sweep gathers soldiers,
   and hulls take no damage from that sweep. The Hailfire and spider had
   shared the same BF1 gun. The Hailfire now takes the BF2 chaingun’s
   50 damage, 0.05-second shot delay and heat values. The spider keeps its
   STAP-cannon numbers. The BF2 ground seat loader also corrects a pre-existing
   class read. It took the first MaxDamage entry, 0, in the inherited
   chaingun ordnance. The original setter processes the second entry, 50,
   last. The ground seat now uses 50. Its 0.1 armor scale yields 5 against
   vehicles. At base damage the gun takes eight hits on a trooper and eleven
   on a heavy.

The pass runs in the BF1 side and map class loader. The named twins and per-class
adjustments are in normalize/vehicles.rs. BF2 class files retain their own
values. The Hailfire read correction and ground beam penetration check
Sim::space_round().

## Named remaining differences

- **Combat speeder paired fire (checked 2026-11-14).** The V-25
  (`all_hover_combatspeeder`) exists in both games. BF1 authors
  `ShotsPerSalvo = 2` for the pilot's `cannon1` blasters and `missile`;
  their BF2 twins use one. The shared rule deliberately sets both BF1
  weapons to one, so consecutive shots alternate their two muzzle points.
  This is the salvo override in `normalize/vehicles.rs`, rather than a
  class-load collision or a muzzle-ring fault. The maintainer confirmed
  BF2 precedence for shared vehicles; keep this pattern. [V] Original BF1
  slot 10, SLUS-20898 / CRC 503BF9E1, converted into a separate emucap
  state: R1 held 8 frames shows both blaster bolts together; independently
  restoring, waiting 180 frames for reload and holding L1 8 frames shows
  both missile launches together. Local captures are
  `engine/artifacts/v25-reference-primary.png` and
  `engine/artifacts/v25-reference-secondary.png` (game data, not committed).
  Static mechanics: [blaster damage](re-blaster-damage.md) lists the BF2
  cannon constructor `FUN_003304e0`'s one-shot default (B2:312370 onward)
  and the per-projectile ring advance `FUN_00301d28` / BF1 `FUN_0026d198`
  (see that note's multi-barrel ring evidence). Port source checked at
  `504729a74dc1` plus the existing dirty vehicle-parity tree; black box
  `last.json`, tat1/con, exit tick 564, identifies the same dirty build.
  No gameplay code changed. BF2 emulator firing comparison remains unchecked.
- **Projectile behavior.** Guns retain BF1 art, sounds, speed, lifetime and
  blast radii. The fighter-tank secondary is a 40 m/s BF1 bullet and an
  80 m/s BF2 missile with zero authored turn rate. The on-foot BF1 torpedo
  retains flat 40 m/s flight and the authored waver on the relevant classes.
  BF2 rockets accelerate from 20 to 100 m/s. Compare impact damage by age.
  The BF1 Hailfire missile keeps its authored seat ammo supply, even though
  its magazine size and reload come from BF2.
- **Impact push.** BF1’s fighter tank light cannons, AAT light cannon, AT-ST
  head blasters, spider blaster and combat speeder light gun keep their
  authored push where the BF2 counterpart has no damaging blast. The
  impact can still knock down a nearby soldier or move a corpse even
  though its splash damage is zero. measure: original impact reactions
  from each gun on living soldiers and corpses.
- **Beam duration.** At 30 Hz, a target under a fighter-tank gunner beam for
  its whole BF1 cast takes about 1,083.33 person damage after the pass
  (1,155.56 before), compared with 1,022.22 for the BF2 cast. The BF1
  spider beam gives about 3,000 on a person (1,333.33 before), and the BF2
  beam gives about 3,066.67. These casts use the same per-shot descriptor
  after the pass, with each original’s LifeSpan and fixed-step damage share.
- **Geonosis sweep width.** The BF1 ray keeps its 3 m radius. BF2’s radius is
  5 m. The damage away from the centre and after earlier victims can differ.
- **Vehicle kits.** The pass copies numbers for existing seats. The BF2
  AAT’s homing missile, the AT-ST’s guided rocket and the Hoth laser-mortar
  turret’s separate mortar are still specific to those kits.
- **Flyers and destruction.** BF1 ground-map flyer weapons and hull health
  follow their prior rules apart from the snowspeeder and the separate
  MTT gun. Vehicle death blasts continue on the existing BF1 soldier scale.
  replace: a ground-map flyer pass. measure:
  shared guns in live rounds. A fast bike still has its own collision and
  ramming rules.
- **Infantry base damage.** Giving light arms the same armor scale preserves
  their authored base damage, pellet counts, and rates. A BF1 rifle with
  base damage 60 takes 6 HP off a vehicle. A BF2 rifle at base damage 80
  takes 8. The BF1 torpedo’s impact blast keeps the prior building scales
  by faction. VehicleScale for this blast remains zero.

## Table evidence

“Before” is openbf at 4562dd6a, after soldier normalization. Both sides of
the gun comparison use 400 HP for a trooper and 550 HP for a heavy. “After”
is the vehicle pass from cb7b5853 plus its integration fixes. The tables
resolve the baked class properties, including map-local classes on all 16 BF1
ground maps in the current bake. The original PS2 class census supplies the
Gian speeder. The calculations use the port’s damage descriptors, its class
loader, and its fixed damage step for beams. They are arithmetic from the
class data. A full-blast count assumes maximum splash, and a repeated beam
assumes a stationary exposed target. Round outcomes and in-game trajectories
require their own checks.

Original game evidence includes the BF1 and BF2 PS2 class census under
engine/crates/manifest/data/gameplay/ps2-v1, the missile updates in
re-ordnance-kinds.md §4.3 (B1:202941–203144, B2:312818–313031), the BF2
beam update FUN_00311d00 (B2:303153–303368), and the ray sweeps FUN_00278e08 (B1:199532–199536) and
FUN_00317f20 (B2:307338–307566). See re-blaster-damage.md §7.3 for age fall-off and
re-weapon-charge.md §4 for the charge stages.

## Health and direct rocket hits

Each row uses the target’s health type. Body-hit counts come first. A parenthesized
count gives human direct weak-part hits. The old torpedo hits a vehicle for 600
at any age. The new one starts at 500 and rises to 1,500 at age 0.5 seconds.
An Alliance torpedo flying straight at 40 m/s travels 20 m in that time. The BF2
rocket travels roughly the same distance while accelerating from 20 m/s. Age is
exact for the calculation and distance varies with flight path and tick timing.

The counts exclude splash, repairs, shields, buffs and the AT-AT tow trip. A
turret takes BuildingScale, which gave 150 damage per direct old torpedo hit.
Animals take AnimalScale. A dash in the health column marks no authored MaxHealth.
A dash in the final column marks no BF2 twin listed by the pass. Gian speeder
rows use the original PS2 class census because the current bake omits it.

| BF1 class | Health type | Health before → after | Rockets before | Rockets at age 0 | Rockets at age ≥ 0.5 s | BF2 twin health |
| --- | --- | --- | --- | --- | --- | --- |
| `all_fly_kwing` | Vehicle | 5000 | 9 | 10 | 4 | — |
| `all_fly_snowspeeder` | Vehicle | 3000 → 1400 | 5 | 3 | 1 | 1400 |
| `all_fly_xwing` | Vehicle | 6600 | 11 | 14 | 5 | — |
| `all_fly_ywing` | Vehicle | 7200 | 12 | 15 | 5 | — |
| `all_hover_combatspeeder` | Vehicle | 5400 → 8000 | 9 (5) | 16 (8) | 6 (3) | 8000 |
| `all_hover_hovernaut` | Vehicle | 12500 → 33333 | 21 (11) | 67 (34) | 23 (12) | — |
| `all_walk_tauntaun` | Animal | 400 | 1 | 1 | 1 | — |
| `bes_bldg_AAA_turret` | Building | 500 → 1333 | 4 | 3 | 1 | — |
| `bes_fly_cloudcar` | Vehicle | 5000 | 9 | 10 | 4 | — |
| `cis_fly_droidfighter` | Vehicle | 5400 | 9 | 11 | 4 | — |
| `cis_fly_fedlander_DOME` | Vehicle | 4000 | 7 | 8 | 3 | — |
| `cis_fly_geofighter` | Vehicle | 4200 | 7 | 9 | 3 | — |
| `cis_fly_maf` | Vehicle | 7200 | 12 | 15 | 5 | — |
| `cis_hover_aat` | Vehicle | 6600 → 8000 | 11 (6) | 16 (8) | 6 (3) | 8000 |
| `cis_hover_mtt` | Vehicle | 15000 → 40000 | 25 (13) | 80 (40) | 27 (14) | — |
| `cis_hover_stap` | Vehicle | 600 → 150 | 1 | 1 | 1 | 150 |
| `cis_tread_hailfire` | Vehicle | 5200 → 8000 | 9 (5) | 16 (8) | 6 (3) | 8000 |
| `cis_walk_spider` | Vehicle | 6000 → 20000 | 10 (3) | 40 (10) | 14 (4) | 20000 |
| `com_weap_gunturret` | Building | 300 → 800 | 2 | 2 | 1 | 800 |
| `geo_bldg_geoturret` | Building | 1250 → 3000 | 9 | 6 | 2 | 3000 |
| `gun_walk_kaadu` | Animal | 400 | 1 | 1 | 1 | — |
| `hoth_bldg_dish_turret` | Building | 600 → 3000 | 4 | 6 | 2 | 3000 |
| `hoth_bldg_hoth_turret` | Building | 750 → 3000 | 5 | 6 | 2 | 3000 |
| `imp_fly_tiebomber` | Vehicle | 7200 | 12 | 15 | 5 | — |
| `imp_fly_tiefighter` | Vehicle | 4800 | 8 | 10 | 4 | — |
| `imp_fly_trooptrans` | Vehicle | 5000 | 9 | 10 | 4 | — |
| `imp_hover_fightertank` | Vehicle | 6600 → 8000 | 11 (6) | 16 (8) | 6 (3) | 8000 |
| `imp_hover_speederbike` | Vehicle | 600 → 80 | 1 | 1 | 1 | 80 |
| `imp_walk_atat` | Vehicle | 30000 → 60000 | 50 (13) | 120 (30) | 40 (10) | 60000 |
| `imp_walk_atst` | Vehicle | 6000 → 8000 | 10 (5) | 16 (8) | 6 (3) | 8000 |
| `imp_walk_atst_jungle` | Vehicle | 6000 → 8000 | 10 (5) | 16 (8) | 6 (3) | 8000 |
| `imp_walk_atst_snow` | Vehicle | 6000 → 8000 | 10 (5) | 16 (8) | 6 (3) | 8000 |
| `nab_hover_gianspeeder` | Vehicle | 5400 → 8000 | 9 | 16 | 6 | — |
| `rep_fly_gunship` | Vehicle | 7800 | 13 | 16 | 6 | — |
| `rep_fly_jedifighter` | Vehicle | 6000 | 10 | 12 | 4 | — |
| `rep_hover_fightertank` | Vehicle | 6600 → 8000 | 11 (6) | 16 (8) | 6 (3) | 8000 |
| `rep_hover_speederbike` | Vehicle | 600 → 80 | 1 | 1 | 1 | 80 |
| `rep_walk_atte` | Vehicle | 15000 → 40000 | 25 (7) | 80 (20) | 27 (7) | 40000 |
| `tat_bldg_barge_turret` | Building | — → 400 | — | 1 | 1 | 400 |
| `tat_hover_landspeeder` | Vehicle | 750 → 100 | 2 | 1 | 1 | — |
| `tat_hover_skiff` | Vehicle | 4200 → 5091 | 7 | 11 | 4 | — |
| `yav_bldg_tower_turret` | Building | 300 → 800 | 2 | 2 | 1 | 800 |

## Ground vehicle, turret and snowspeeder guns

Damage is direct at age zero. A plus gives maximum centre splash, shown separately.
The hit columns count direct hits on an unshielded Person with no repairs, buffs,
friendly-fire reduction or criticals. A square-bracketed count includes full
splash on the same target, when that changes the count. Distance from the
blast can make the real count higher. Slashes separate the Hoth dish’s four
authored charge points. A fully charged dish shot reaches 1,800 direct damage.
The first soldier on a full-charge Geonosis ray takes the centre
value. Beam hit counts assume a target on the line for the whole cast. Beam
damage columns give class descriptors. A dash in a hit column means the
listed direct attack cannot kill the target.

| BF1 gun | Vehicle damage before → after | Trooper damage before → after | Hits on 400 HP | Hits on 550 HP |
| --- | --- | --- | --- | --- |
| `all_weap_fly_snowspeeder_cannon` | 40 | 533.33 + 133.33 → 400 | 1 | 2 [1] → 2 |
| `all_weap_fly_snowspeeder_reargun` | 0 | 0 | — | — |
| `all_weap_hover_combatspeeder_cannon` | 800 → 500 | 1066.67 + 533.33 → 500 + 125 | 1 | 1 → 2 [1] |
| `all_weap_hover_combatspeeder_cannon1` | 54 → 15 | 400 + 400 → 150 | 1 → 3 | 2 [1] → 4 |
| `all_weap_hover_combatspeeder_missile` | 160 → 1500 | 533.33 + 400 → 1500 + 150 | 1 | 2 [1] → 1 |
| `all_weap_hover_hovernaut_blaster` | 315 → 381.82 | 600 + 266.67 | 1 | 1 |
| `all_weap_hover_hovernaut_cannon1` | 800 → 969.7 | 1600 | 1 | 1 |
| `all_weap_hover_hovernaut_missile` | 24 → 29.09 | 160 + 133.33 | 3 [2] | 4 [2] |
| `cis_weap_fly_maf_cannon` (MTT) | 300 → 363.64 | 400 + 400 | 1 | 2 [1] |
| `cis_weap_hover_aat_cannon` | 150 → 20 | 533.33 + 400 → 200 | 1 → 2 | 2 [1] → 3 |
| `cis_weap_hover_aat_launcher` | 1200 → 2000 | 1600 + 533.33 → 2000 + 400 | 1 | 1 |
| `cis_weap_hover_stap_cannon` | 30 → 25 | 400 + 26.67 → 250 | 1 → 2 | 2 → 3 |
| `cis_weap_tread_hailfire_missile` | 150 → 1500 | 400 + 533.33 → 1500 + 300 | 1 | 2 [1] → 1 |
| `cis_weap_walk_spider_cannon` | 40 → 25 | 533.33 + 533.33 → 250 | 1 → 2 | 2 [1] → 3 |
| `cis_weap_walk_spider_cannon` (Hailfire) | 40 → 5 | 533.33 + 533.33 → 50 | 1 → 8 | 2 [1] → 11 |
| `cis_weap_walk_spider_laser` | 1000 → 1500 | 1333.33 → 3000 | 1 | 1 |
| `com_weap_bldg_gunturret` | 100 → 10 | 133.33 + 33.33 → 100 | 3 → 4 | 5 [4] → 6 |
| `geo_weap_bldg_geoturret` | 0 | 20 → 400 | 20 → 1 | 28 → 2 |
| `hoth_weap_bldg_dishturret_cannon` | 200/300/400/500 → 1000/1250/1500/3000 + 400 | 266.67/400/533.33/666.67 + 400 → 1000/1250/1500/3000 + 400 | 2 [1]/1/1/1 → 1/1/1/1 | 3 [1]/2 [1]/2 [1]/1 → 1/1/1/1 |
| `hoth_weap_bldg_hothturret_cannon` | 75 → 30 | 400 → 300 | 1 → 2 | 2 |
| `imp_weap_hover_fightertank_cannon` | 57 → 20 | 400 + 400 → 200 | 1 → 2 | 2 [1] → 3 |
| `imp_weap_hover_fightertank_laser` | 800 → 500 | 1066.67 → 1000 | 1 | 1 |
| `imp_weap_hover_fightertank_missile` | 220 → 1500 | 533.33 + 400 → 1500 + 400 | 1 | 2 [1] → 1 |
| `imp_weap_hover_speederbike_cannon` | 30 → 15 | 400 → 150 | 1 → 3 | 2 → 4 |
| `imp_weap_walk_atat_cannon_chin` | 2500 → 5000 + 40 | 3333.33 + 1066.67 → 5000 + 400 | 1 | 1 |
| `imp_weap_walk_atat_cannon_head` | 400 → 1000 | 533.33 + 533.33 → 1000 + 50 | 1 | 2 [1] → 1 |
| `imp_weap_walk_atst_cannon_chin` | 225 → 1000 | 600 + 533.33 → 1000 + 400 | 1 | 1 |
| `imp_weap_walk_atst_cannon_head` | 200 → 20 | 533.33 + 533.33 → 200 | 1 → 2 | 2 [1] → 3 |
| `kam_weap_bldg_AAA_turret_cannon` | 800 → 969.7 | 1066.67 + 400 | 1 | 1 |
| `nab_weap_hover_gianspeeder_cannon` | 800 → 969.7 | 1066.67 + 400 | 1 | 1 |
| `nab_weap_hover_gianspeeder_cannon1` | 75 → 90.91 | 400 | 1 | 2 |
| `rep_weap_hover_fightertank_cannon` | 57 → 20 | 400 + 533.33 → 200 | 1 → 2 | 2 [1] → 3 |
| `rep_weap_hover_fightertank_laser` | 800 → 500 | 1066.67 → 1000 | 1 | 1 |
| `rep_weap_hover_fightertank_missile` | 220 → 1500 | 533.33 + 400 → 1500 + 400 | 1 | 2 [1] → 1 |
| `rep_weap_hover_speederbike_cannon` | 12.5 + 100 → 10 | 66.67 + 133.33 → 100 | 6 [2] → 4 | 9 [3] → 6 |
| `rep_weap_walk_atte_cannon` | 160 → 100 | 533.33 + 533.33 → 500 + 50 | 1 | 2 [1] |
| `rep_weap_walk_atte_cannon_heavy` | 160 → 1250 | 533.33 + 533.33 → 1250 + 400 | 1 | 2 [1] → 1 |
| `rep_weap_walk_atte_turretcannon` | 1200 → 2000 | 1600 + 533.33 → 2000 + 400 | 1 | 1 |
| `yav_weap_bldg_tower_minigun` | 100 → 10 | 133.33 → 100 | 3 → 4 | 5 → 6 |

## Deferred ground-map flyer guns

The same conventions apply. Aircraft health and gun numbers in these rows
follow the existing BF1 ground-map rule. The MTT has its own copy of the
MAF cannon, listed above.

| BF1 gun | Vehicle damage before → after | Trooper damage before → after | Hits on 400 HP | Hits on 550 HP |
| --- | --- | --- | --- | --- |
| `all_weap_fly_kwing_bomb` | 600 + 600 | 80 + 1000 | 5 [1] | 7 [1] |
| `all_weap_fly_kwing_conc` | 100 | 266.67 + 66.67 | 2 | 3 [2] |
| `all_weap_fly_kwing_laser` | 250 + 50 | 333.33 + 133.33 | 2 [1] | 2 |
| `all_weap_fly_kwing_miss` | 150 + 10 | 200 + 133.33 | 2 | 3 [2] |
| `all_weap_fly_xwing_cannon` | 600 | 800 + 133.33 | 1 | 1 |
| `all_weap_fly_xwing_ptorpedo` | 1500 | 2000 + 133.33 | 1 | 1 |
| `all_weap_fly_ywing_bomb` | 600 + 600 | 800 + 1000 | 1 | 1 |
| `all_weap_fly_ywing_cannon` | 600 | 800 + 133.33 | 1 | 1 |
| `bes_weap_cloudcar_miss` | 50 + 10 | 66.67 + 133.33 | 6 [2] | 9 [3] |
| `bes_weap_fly_cloudcar_cannon` | 200 + 25 | 266.67 + 133.33 | 2 [1] | 3 [2] |
| `cis_weap_fly_droidfighter_cannon` | 800 | 1066.67 + 400 | 1 | 1 |
| `cis_weap_fly_geofighter_cannon` | 180 | 400 + 400 | 1 | 2 [1] |
| `cis_weap_fly_maf_cannon` | 300 | 400 + 400 | 1 | 2 [1] |
| `cis_weap_fly_maf_laser` | 1000 | 1333.33 | 1 | 1 |
| `cis_weap_fly_maf_torpedo` | 800 | 1066.67 + 400 | 1 | 1 |
| `imp_weap_fly_tiebomber_bomb` | 600 + 750 | 800 + 1000 | 1 | 1 |
| `imp_weap_fly_tiebomber_ptorpedo` | 1200 | 1600 + 400 | 1 | 1 |
| `imp_weap_fly_tiefighter_cannon` | 800 | 1066.67 + 133.33 | 1 | 1 |
| `rep_weap_fly_gunship_ball` | 150 | 800 | 1 | 1 |
| `rep_weap_fly_gunship_cannon` | 60 | 400 + 400 | 1 | 2 [1] |
| `rep_weap_fly_gunship_missile` | 160 | 533.33 + 400 | 1 | 2 [1] |
| `rep_weap_fly_gunship_wing` | 450 | 600 | 1 | 1 |
| `rep_weap_fly_jedifighter_cannon` | 400 | 533.33 + 400 | 1 | 2 [1] |
| `rep_weap_fly_jedifighter_missile` | 1000 | 1333.33 + 400 | 1 | 1 |

## Infantry ordnance and blasts

This inventory covers damage-bearing BF1 infantry ordnance and explosions in
the baked side and map class files. A blast gives its centre damage, the
torpedo gives its age-zero damage, and each class gives damage per bolt, pellet
or blast. A slash gives an emitter’s low and high endpoints. The BF2 donor kits
and award weapons from [the soldier pass](gameplay-normalization.md) use BF2 data.

| BF1 class | Vehicle damage before → after | Trooper damage before → after | Building damage before → after |
| --- | --- | --- | --- |
| `all_weap_inf_bowcaster_ord` | 0 → 6 | 80 | 3 → 6 |
| `all_weap_inf_bowcaster_ord_high` | 0 → 7.5 | 100 | 3.75 → 7.5 |
| `all_weap_inf_crackgrenade_exp` | 400 → 484.85 | 133.33 | 200 |
| `all_weap_inf_mortar_launcher_exp` | 0 → 400 | 400 | 32 → 400 |
| `all_weap_inf_mortar_launcher_ord` | 0 → 100 | 0 → 100 | 0 → 100 |
| `all_weap_inf_orbital_attack_salvo_exp` | 0 | 666.67 | 0 |
| `all_weap_inf_orbital_attack_salvo_ord` | 1000 | 1333.33 | 1000 |
| `all_weap_inf_pistol_ord` | 0 → 4 | 60 | 2 → 4 |
| `all_weap_inf_remote_charge_dispenser_exp` | 2000 → 3000 | 2666.67 → 1200 | 1000 → 2250 |
| `all_weap_inf_remotedroid_blaster_ord` | 0 → 1 | 60 | 0 → 1 |
| `all_weap_inf_remotedroid_exp2` | 25 | 66.67 | 50 |
| `all_weap_inf_rifle_ord` | 0 → 6 | 80 | 6 |
| `all_weap_inf_shotgun_ord` | 0 → 6.5 | 86.67 | 3.25 → 6.5 |
| `all_weap_inf_sniperrifle_ord` | 0 → 16 | 300 | 16 |
| `all_weap_inf_thermaldetonator_exp` | 80 → 450 | 450 | 80 → 450 |
| `all_weap_inf_torpedo_launcher_exp` | 0 | 400 | 80 |
| `all_weap_inf_torpedo_launcher_ord` | 600 → 500 | 800 → 500 | 150 → 500 |
| `cis_weap_inf_crackgrenade_exp` | 400 → 484.85 | 133.33 | 200 |
| `cis_weap_inf_mortar_launcher_exp` | 0 → 400 | 400 | 37.5 → 400 |
| `cis_weap_inf_mortar_launcher_ord` | 0 → 100 | 0 → 100 | 0 → 100 |
| `cis_weap_inf_mortar_launcher_ord1` | 15 → 400 | 200 → 400 | 15 → 400 |
| `cis_weap_inf_orbital_attack_salvo_exp` | 0 | 666.67 | 0 |
| `cis_weap_inf_orbital_attack_salvo_ord` | 0 | 1333.33 | 0 |
| `cis_weap_inf_pistol_ord` | 0 → 4 | 60 | 2 → 4 |
| `cis_weap_inf_remotedroid_blaster_ord` | 0 → 1 | 60 | 0 → 1 |
| `cis_weap_inf_remotedroid_exp2` | 25 | 66.67 | 50 |
| `cis_weap_inf_repeater_ord` | 0 → 5.5 | 73.33 | 5.5 |
| `cis_weap_inf_rifle_ord` | 0 → 6 | 80 | 6 |
| `cis_weap_inf_sniperrifle_ord` | 0 → 16 | 300 | 16 |
| `cis_weap_inf_thermaldetonator_exp` | 80 → 450 | 450 | 80 → 450 |
| `cis_weap_inf_torpedo_launcher_exp` | 0 | 400 | 0 |
| `cis_weap_inf_torpedo_launcher_ord` | 600 → 500 | 800 → 500 | 150 → 500 |
| `cis_weap_inf_wrist_blaster_ord` | 0 → 6 | 80 | 6 |
| `cis_weap_inf_wrist_rocket_exp` | 25 | 133.33 | 25 |
| `cis_weap_inf_wrist_rocket_ord` | 200 | 266.67 | 200 |
| `com_weap_inf_empmine_exp` | 400 | 100 | 0 |
| `com_weap_inf_landmine_exp` | 1750 → 12000 | 2333.33 → 8000 | 175 → 800 |
| `com_weap_inf_seismic_mine_exp` | 5000 | 5000 | 500 |
| `gun_weap_inf_haywiredetonator_exp` | 240 → 290.91 | 400 | 150 |
| `imp_weap_inf_crackgrenade_exp` | 400 → 484.85 | 133.33 | 200 |
| `imp_weap_inf_mortar_launcher_exp` | 0 → 400 | 400 | 40 → 400 |
| `imp_weap_inf_mortar_launcher_ord` | 0 → 100 | 0 → 100 | 0 → 100 |
| `imp_weap_inf_mortar_launcher_ord1` | 15 → 400 | 200 → 400 | 15 → 400 |
| `imp_weap_inf_orbital_attack_salvo_exp` | 0 | 666.67 | 0 |
| `imp_weap_inf_orbital_attack_salvo_ord` | 1000 | 1333.33 | 1000 |
| `imp_weap_inf_pistol_ord` | 0 → 4 | 60 | 2 → 4 |
| `imp_weap_inf_remotedroid_blaster_ord` | 0 → 1 | 60 | 0 → 1 |
| `imp_weap_inf_remotedroid_exp2` | 25 | 66.67 | 50 |
| `imp_weap_inf_rifle_ord` | 0 → 6 | 70 | 6 |
| `imp_weap_inf_shotgun_ord` | 0 → 6.5 | 86.67 | 3.25 → 6.5 |
| `imp_weap_inf_sniperrifle_ord` | 0 → 16 | 300 | 16 |
| `imp_weap_inf_thermaldetonator_exp` | 80 → 450 | 450 | 80 → 450 |
| `imp_weap_inf_torpedo_launcher_exp` | 0 | 400 | 80 |
| `imp_weap_inf_torpedo_launcher_ord` | 600 → 500 | 800 → 500 | 150 → 500 |
| `rep_weap_inf_arccaster_ord` | 0 | 133.33/333.33 | 0 |
| `rep_weap_inf_commando_pistol_ord` | 0 → 4 | 60 | 2 → 4 |
| `rep_weap_inf_crackgrenade_exp` | 400 → 484.85 | 133.33 | 200 |
| `rep_weap_inf_haywiredetonator_exp` | 0 | 300 | 45 |
| `rep_weap_inf_orbital_attack_salvo_exp` | 0 | 666.67 | 0 |
| `rep_weap_inf_orbital_attack_salvo_ord` | 1000 | 1333.33 | 1000 |
| `rep_weap_inf_pistol_ord` | 0 → 4 | 60 | 2 → 4 |
| `rep_weap_inf_remotedroid_blaster_ord` | 0 → 1 | 60 | 0 → 1 |
| `rep_weap_inf_remotedroid_exp2` | 25 | 66.67 | 50 |
| `rep_weap_inf_rifle_ord` | 0 → 6 | 80 | 6 |
| `rep_weap_inf_rocket_launcher_exp` | 0 | 200 | 60 |
| `rep_weap_inf_rocket_launcher_ord` | 0 | 80 | 12 |
| `rep_weap_inf_sniperrifle_ord` | 0 → 16 | 300 | 16 |
| `rep_weap_inf_thermaldetonator_exp` | 80 → 450 | 450 | 80 → 450 |
| `rep_weap_inf_torpedo_launcher_exp` | 0 | 400 | 80 |
| `rep_weap_inf_torpedo_launcher_ord` | 600 → 500 | 800 → 500 | 150 → 500 |


## Headless round checks

The locked-versus-aimed test fired at thirteen vehicles from stands chosen at
25–30 m. A locked count is trigger pulls, including a paired salvo where
authored. An aimed count is individual rounds that struck the weak part.

| BF1 vehicle | Locked pulls before → after | Aimed hits before → after |
| --- | --- | --- |
| Republic fighter tank | 6 → 3 | 6 → 3 |
| Hailfire | 5 → 3 | 5 → 3 |
| Snow AT-ST | 3 → 2 | 5 → 3 |
| Spider walker | 3 → 4 | 3 → 4 |
| AT-TE | 7 → 7 | 7 → 7 |
| AT-AT | 10 → 8 | 13 → 10 |

The before columns use the pre-pass fixtures. In the new Snow AT-ST
check, the first pull fired two rounds and took 6,000 of 8,000 HP.
The first attempt at re-pinning counted a shot from an earlier stand during
the next lock dwell. The fixture now restores full health immediately before
the first counted trigger press. Its seven BF2 witnesses retained their
counts.

The awards-off Good proxy played both sides of three BF1 GCW maps at
Normal and Hard, seeds 1–32. Normal stayed at 156/192 wins (81.2%). Hard
fell from 95/192 (49.5%, Wilson 95% 42.5–56.5) to 81/192 (42.2%,
35.4–49.3). The Empire's Hard result was 27/96 → 21/96, and the Rebels'
was 68/96 → 60/96. The observed Rebel lead changed from 42.7 to 40.6
percentage points. Resampling matched seeds gave a 95% interval of
−16.7 to +12.5 points for the change. Both Coruscant CW sides repeated
their outcomes exactly, as did both Coruscant GCW sides. Including
Coruscant GCW in the as-baked gate gave 139/256 → 125/256 on Hard
(54.3% → 48.8%). Normal stayed at 204/256 (79.7%).

The faction census build reproduced the as-baked Normal and Hard rounds
exactly. Pooling its flipped assignments with the as-baked assignments gave an
Empire Hard result of 49/192 on both builds. The Rebels moved from
119/192 to 105/192. The Hard gap moved from 36.5 to 29.2 points with
a paired 95% change interval of −19.3 to +4.7. The Normal gap moved
from 21.4 to 9.9 points with a paired interval of −20.8 to −2.6.

Bots finished all 204 default and 162 alternate-era rounds on both builds
(Normal, seeds 42–47). Median round ticks moved from 13,031 to 12,885 in
the default block and from 13,590 to 13,605 in the alternate block.
The default census recorded 6 → 2 recycles and 26 → 27 relocations.
The shift in kills per foot-minute exceeded the baseline's seed-block band
(−0.012 against 0.008). Both space goldens matched at all checkpoints.
The alternate block's gameplay metrics stayed inside their seed noise
bands. The BF2 Geonosis Hailfire correction moved three of six default
hashes there. The other BF2 sweep hashes matched.
All 12 BF1 proxy goldens moved and repeated their new hashes on rerun.
The TAT1 walk and aim replays, aim-target replay, and 600-tick fixture
trace matched byte for byte. The 100-tick fixture kept `5573f2b603ed95f4`.
Original-game play and controller feel remain unchecked.
