# One set of soldier health and damage numbers for both games

Status: [ours], PROVISIONAL. measure: BF1-map win rate and kills a minute for the frozen
stand-in human against the pre-change baseline, and the same on the BF2 twin map.

The maintainer (2026-11-10): "I want this to be one game, not 2 games in 1 keeping the games
separate - one seamless game. ... make health for each class equivalent across the 2 games ...
rifles do the same damage in bf1 as they do in bf2, and it has the same effect because health
is the same."

Before this, each map played with its own disc's numbers: a trooper had 300 health and a
60 rifle on a BF1 map and 400 health and an 80 rifle on a BF2 map, and the BF2 weapons
BF1's droids borrow (`roster::WEAPON_DONORS`) met 300-health clones.

## The reference scale

BF2's numbers are the reference: a trooper has 400 health and a rifle bolt does 80.

- BF1's trooper and rifle are exactly 3/4 of BF2's (300 and 60), so BF1's baseline converts
  without loss: five body hits stay five.
- BF2 has the larger roster, real hero health and the award weapons.
- The port already uses BF2's HUD and jets on every map, and BF1's droids already carry BF2
  weapons.
- BF2's class files, and with them every space unit and weapon, are not touched.

## Where it happens

One pass, `game::normalize::apply`, rewrites each BF1 class definition as it is read:
`ClassDb::load` for the files named `bf1_*`, and `ClassDb::load_map` for the `classes.ron` of
a map whose manifest says `source_game: "bf1"` (it holds the map's own vehicles, turrets and
props). BF1 classes never inherit from one another, so one definition at a time is exact.
`ClassDb::disc()` reads the disc's own numbers (tests and the tables below). BF2's class
files and a BF2 map's `classes.ron` are read as written.

## The rules

1. **Soldier health** (`normalize::HEALTH`). A BF1 soldier class takes its BF2 counterpart's
   `MaxHealth`, the Gungans included (BF2's are 200). A class BF2 had no counterpart for
   would be multiplied by 4/3, keeping its strength relative to its own game's trooper. BF1's four stand-in heroes
   (1000000 health, fielded by no mission) are left alone.
2. **Damage to soldiers** (`normalize::PERSON_DAMAGE`). Every BF1 ordnance and explosion class
   named `*_weap_inf_*` (every damage class a BF1 soldier or native can reach is named so; a
   test walks the class graph) has its `PersonScale`, `AnimalScale`, `DroidScale`,
   `HealthScale` and `ShieldScale` multiplied by one factor: the one that lands its damage to
   a person on its BF2 counterpart's, or 4/3 when it has none. `MaxDamage`, `VehicleScale`
   and `BuildingScale` stay as written, so damage to vehicles and buildings does not change.
   BF1's arc caster is the one exception: BF1's emitter ignores the scales, so its `MaxDamage`
   and `Damage` are multiplied instead (and its damage to vehicles rises by 4/3 with them).
3. **Keys taken whole from BF2** (`normalize::KEYS`). A weapon that takes BF2's damage takes
   BF2's rate of fire, so it is the same weapon on every map: the sniper rifles (1.0 s), the
   jet trooper's pistol (0.1 s), the Tusken rifle (0.33 s) and sniper (1.0 s), the Geonosian's
   blaster (1.0 s). The torpedo launchers lock in BF2's 1.0 s (BF1's took 1.5 s). The rocket blast takes BF2's radii (2 m inner, 4 m outer). The recon droid
   has BF2's 100 health. A health droid heals 25 a pass. The fusion cutter reaches 4 m in a
   90 degree cone, for repairing and slicing (`award-weapons.md` section 4).
4. **Pickups.** A BF1 bacta tank or pack heals 4/3 as much, the same share of a trooper as on
   its disc (see "Left as they are").
5. **Every other BF1 gun and blast, INTERIM.** Vehicle and flyer guns, building turrets
   (those in a map's own class file too) and death blasts have the same four scales
   multiplied by 4/3 and nothing else, so each kills the shared trooper in as many hits as it
   killed BF1's 300-health one: BF1's vehicles are as strong against soldiers as they were.
   Their damage to vehicles and buildings is unchanged. replace: the vehicle pass that sets
   vehicle health and vehicle guns together for both games (see "Left as they are").

Award weapons are BF2's own classes, so they are the reference and have no rows: pistol 150,
rifle 150, sniper 600, shotgun pellet 85 to a person. A BF1 class carries them from BF2's
class data (`award-weapons.md` section 3), as BF1's droids read their BF2 weapons, and they
arrive on this scale.

Reading damage: `OrdnanceClass::max_damage` is still the disc's number (60 for BF1's rifle);
what a hit takes off a soldier is `max_damage x scales[health type]`. The AI and the space
balance already multiply; a new reader must too.

## Soldier health

| Role | BF1 disc | BF2 | Every map now |
|---|---|---|---|
| Trooper (soldier, stormtrooper, clone, super battle droid) | 300 | 400 | 400 |
| Heavy (vanguard, shock trooper, ARC trooper, assault droid) | 300 | 550 | 550 |
| Sniper (marksman, scout trooper, sharpshooter, assassin droid) | 240 | 300 | 300 |
| Pilot / engineer | 300 | 400 | 400 |
| Wookiee smuggler / Wookiee warrior | 450 | 800 | 800 |
| Dark trooper | 300 | 300 | 300 |
| Jet trooper | 240 | 300 | 300 |
| Droideka | 450 | 550 | 550 |
| Officers (BF2 only) | - | 300 Bothan, 400 others | unchanged |
| Kashyyyk's Wookiees (natives) | 300 | 300 | 300 |
| Ewok | 75 | 200 | 200 |
| Geonosian | 125 | 125 | 125 |
| Tusken raider | 300 | 400 | 400 |
| Tusken hunter | 200 | 300 | 300 |
| Jawa | 200 | 250 | 250 |
| Gungan | 125 | 200 | 200 |
| BF1's stand-in heroes | 1000000 | - | left alone |
| BF2's heroes | - | 1200 to 2400 | unchanged |

What moves on BF1 maps beyond the plain 4/3: a heavy is now tougher than a trooper (7 rifle
hits, not 5), the Wookiee smuggler is much tougher (10 hits, not 8 on the old scale), and the
dark trooper, which had a trooper's health in BF1, is now a quarter under it, as in BF2.

The full per-class table is at the end.

## Weapons: damage to a person and body hits on the game's own trooper

"Hits" is rounds that land on the body; a head hit is x3 in both games.

| Weapon | BF1 disc | Hits on 300 | BF2 | Hits on 400 | BF1 maps now | Hits on 400 | Rule |
|---|---|---|---|---|---|---|---|
| Blaster rifle (Rebel, Republic, CIS) | 60 at 0.2 s | 5 | 80 at 0.2 s | 5 | 80 at 0.2 s | 5 | x4/3 = BF2's |
| Imperial blaster rifle | 60 at 0.2 s | 5 | 70 at 0.166 s | 6 | 70 at 0.166 s | 6 | BF2's, with its rate |
| Pistol | 40 | 8 | 60 | 7 | 60 | 7 | BF2's |
| Jet trooper's pistol | 40 at 0.05 s | 8 | 60 at 0.1 s | 7 | 60 at 0.1 s | 7 | BF2's, with its rate |
| Sniper rifle | 160 at 0.5 s | 2 | 300 at 1.0 s | 2 | 300 at 1.0 s | 2 | BF2's, with its rate |
| Shotgun, a pellet | 65 x 5 at 0.75 s | 5 | 65 x 8 at 1.0 s | 7 | 86.67 x 5 at 0.75 s | 5 | x4/3 |
| Bowcaster | 60 (charged 75) | 5 | 75 x 7 (charged 300) | 6 | 80 (charged 100) | 5 | x4/3 |
| Rocket, direct hit | 600 | 1 | 500 rising to 1500 | 1 | 800 | 1 | x4/3 |
| Rocket, blast | 320, radius 0.75 / 4 m | 1 | 400, radius 2 / 4 m | 1 | 400, radius 2 / 4 m | 1 | BF2's, with its radii |
| Thermal detonator | 320 | 1 | 450 | 1 | 450 | 1 | BF2's |
| Mortar / grenade launcher blast | 300 to 320 | 1 | 400 | 1 | 400 | 1 | BF2's |
| Mortar shell, direct hit | 150 | 2 | 400 | 1 | 200 | 2 | x4/3 |
| Concussion grenade (BF1 only) | 100 | 3 | - | - | 133.33 | 3 | x4/3 |
| EMP grenade (BF1 only), person / droid | 225 / 450 | 2 / 1 | - | - | 300 / 600 | 2 / 1 | x4/3 |
| Arc caster (BF1's clone pilot) | 100 to 250 | 2 | - (BF2's dark trooper: 300 to 325) | - | 133.33 to 333.33 | 2 | x4/3 |
| Jet trooper's EMP launcher, direct / blast to a droid | 120 / 300 | 1 | 200 / 400 | 1 | 160 / 400 | 1 | x4/3 |
| Land mine | 1750 | 1 | 8000 | 1 | 2333.33 | 1 | x4/3 |
| Time bomb | 2000 | 1 | 5000 | 1 | 2666.67 | 1 | x4/3 |
| Recon droid's blaster | 10 | 30 | 60 | 7 | 60 | 7 | BF2's |
| Geonosian's blaster | 25 at 2.0 s | 12 | 100 at 1.0 s | 4 | 100 at 1.0 s | 4 | BF2's, with its rate |
| Tusken rifle | 40 at 0.2 s | 8 | 80 at 0.33 s | 5 | 80 at 0.33 s | 5 | BF2's, with its rate |
| Tusken sniper | 160 at 0.5 s | 2 | 200 at 1.0 s | 2 | 200 at 1.0 s | 2 | BF2's, with its rate |
| Ewok spear | 150 | 2 | 200 | 2 | 200 | 2 | BF2's |
| Ewok rock | 50 | 6 | 50 (+25 blast) | 6 | 66.67 | 6 | x4/3 |
| Gungan grenade (BF1 only) | 300 | 1 | - | - | 400 | 1 | x4/3 |
| Super battle droid's wrist blaster (BF2's on both) | (60) | (5) | 100 | 4 | 100 | 4 | already BF2's |
| Droideka's repeater (BF2's on both) | (55) | (6) | 80 | 5 | 80 | 5 | already BF2's |
| Award pistol / rifle / sniper / shotgun pellet (BF2 only) | - | - | 150 / 150 / 600 / 85 | 3 / 3 / 1 / 5 | unchanged | | reference |

This settles the open question about the BF2 weapons BF1's droids carry: against the old
300-health clone the wrist blaster killed in 3 body hits and the repeater in 4; against the
shared 400-health clone they take 4 and 5 on every map, as in BF2.

What moves on BF1 maps beyond the plain 4/3:

- The Imperial rifle is BF2's: 70 a bolt every 0.166 s (6 body hits, 422 damage a second)
  where the other sides' rifle does 80 every 0.2 s (5 hits, 400 a second).
- A sniper's body shot kills a 300-health class (sniper, dark trooper, jet trooper) in one,
  and snipers fire half as fast.
- A thermal detonator no longer kills a heavy outright (450 against 550), as in BF2.
- The rocket's blast is full out to 2 m instead of 0.75 m (also a little more splash on
  buildings, whose scale is 0.25).
- The recon droid is a real weapon: 60 a hit and 100 health, from 10 and 40.
- Geonosians and Ewoks are BF2's: a Geonosian hits for 100 a second, an Ewok has 200 health.
- A heavy (550) and a Wookiee smuggler (800) gained more than 4/3, so some guns need one
  more hit on them than on the disc: the AT-ST's and AT-AT's head guns and the Geonosis
  turret (400 on the disc, 533 now) take two on a heavy.

## Other survival numbers

Already one rule on every map, no change:

- Fall: one lethal landing speed, 25 m/s (`core::sim::state::LETHAL_LANDING_SPEED`).
- Drowning: 30 a second (`core::sim::water`).
- Head hit: x3 (`core::sim::combat::HEADSHOT_SCALE`).
- The droideka's health and shield come from BF2's class on every map.
- Tauntaun and kaadu have 400 health in both games; a BF1 rifle now takes 5 hits, as BF2's.

Made one rule here:

- A health droid heals 25 a pass (BF1's disc: 15).
- A recon droid has 100 health (BF1's disc: 40).

Scaled, not yet one rule:

| Pickup | BF1 disc | BF1 maps now | BF2 |
|---|---|---|---|
| Small bacta (`health25`) | 50 | 66.67 | 100 |
| Large bacta (`health100`) | 75 | 100 | 200 |
| Health and ammo pack (`dual`) | 100 | 133.33 | 200 |

## Left as they are (named differences)

- **Pickups and drop tables.** BF1's trooper drops ammo 0.40, large bacta 0.30, pack 0.10;
  BF2's drops ammo 0.25, small 0.15, large 0.08, pack 0.08 and its buff and energy items.
  Healing per kill is about 11 % of a trooper in both. Giving BF1's items BF2's amounts
  without BF2's odds would nearly double healing on BF1 maps, so the items are scaled by 4/3
  and the odds kept. One rule here means one drop table too. replace: the maintainer's
  choice between this and BF2's amounts with BF2's odds for the four shared items.
- **Vehicle and turret guns against soldiers, INTERIM.** BF1's are scaled by 4/3 against
  soldiers (rule 5), not made equal to BF2's. The same-named guns differ both ways between
  the games (to a person, BF1 now against BF2: fighter tank cannon 400 against 200, Imperial
  speeder bike 400 against 150, STAP 400 against 250, AAT cannon 533 against 200; but
  Republic speeder bike 67 against 100 and AT-ST chin gun 600 against 1000), and 33
  same-named vehicle and death-blast classes were identical on the discs. So a BF1 light
  cannon still kills a trooper in one hit where BF2's takes two or three. Vehicle health and
  vehicle guns want one pass of their own. replace: that vehicle pass.
  - The Geonosis turret's wide ray reads no scales (`core::sim::fatray`), so only its direct
    hit is scaled (533); the ray's own 400 still kills a trooper in one.
  - The scale keeps every hit count on a trooper
    (`normalization::the_scale_keeps_every_hit_count_on_a_trooper`) except the 20-damage
    blasts (light vehicles, droids, the STAP cannon): 26.67 is not exact in the running health,
    so the sixteenth hit kills, not the fifteenth.
- **A vehicle running a soldier over** deals the vehicle class's own collision damage; the
  soldier's larger health makes a glancing hit a little less deadly on BF1 maps.
- **Blaster fall-off and bolt speed.** BF1's bolts keep their damage at any range and fly at
  80 m/s; BF2's fly at 160 m/s and fall from 80 to 50 late in flight. Left: speed and range
  are the weapon's character, and the slower bolt already makes long shots harder.
- **Shotgun.** BF1's 5 pellets every 0.75 s at 86.67 (433 a volley) against BF2's 8 every
  1.0 s at 65 (520 a volley, with fall-off): about 11 % more damage a second on BF1 maps.
- **Bowcaster.** BF1's single bolt and BF2's seven-bolt fan are different weapons.
- **Clip sizes, reload times, heat and spread** are each game's own.
- **Damage to vehicles and buildings** from infantry weapons is each game's own (BF1's
  thermal detonator does a quarter of its damage to a vehicle, BF2's all of it).
- **The landing-damage law** of the originals is not ported (`re-soldier-ground.md` sections 4, 8).
- **The fallback trooper** (`WalkSim::default_trooper`, used with no class data) keeps 300
  health and the 60 rifle: it is self-consistent and keeps `--sim-hash` stable.
- **BF1's stand-in heroes.**

## Hashes

Every BF1 round's state hash moves from the first tick (soldier health). BF2 ground rounds
and every space round are unchanged: no BF2 class file and no BF2 map's own class file is
rewritten (`normalization::bf2_class_files_are_untouched`,
`normalization::a_maps_own_classes_follow_its_game`).

Measured 2026-11-10 in two steps, each a release binary before and after, `--sim-trace <out>
<ticks> --map <dir> --mission <ron> --state-hashes`, the trace and every tick's hash
compared. Step 1 is main against the first build (health, infantry damage); step 2 is that
build against this one (the Imperial rifle, rule 5, a BF1 map's own class file).

| Missions | Step | Ticks | Result |
|---|---|---|---|
| All 33 BF2-source conquest missions (`bf2_*_con`, `bf2_*_con_cw`, `tat2_con`, `tat2_con_cw`) | 1 and 2 | 15000 | byte-identical both times |
| `bf1_kas1_con_cw`, `bf1_hot1_con`, `tat1_con` | 1 | 3600 | differ from tick 1 |
| All 28 BF1-source conquest missions | 2 | 15000 | all differ: the 14 with an Imperial side from tick 1, the 14 Clone Wars ones from the first vehicle, turret or blast hit on a soldier (tick 218 to 10180) |

`scripts/sim_gates.sh` (the references in `engine/artifacts/loop/gates.toml` are the
maintainer's to refresh; they are not edited here):

| Gate | Main | Now |
|---|---|---|
| `--sim-hash 600` (built-in fixture) | `5c5458c8927f122b` | unchanged |
| `--aim-replay` (scripted targets) | `443de283b68845b0`, motion identical | unchanged |
| walk demo, 407 ticks on TAT1 | `93b8423fb731a308` | `474c5cce53472502` |
| aim demo, 120 ticks on TAT1 | `9fa0ea8fa73eb0bb` | `ac5bf2e68e45f19d` |

The two TAT1 `motion.csv` files differ in the `health` column alone (300 becomes 400); the
position, stance, shots, hits and kills columns are byte-identical. Step 2 moved none of the
four gates and none of their `motion.csv` files. The all-map outcome sweep,
the leak census and the fidelity numbers of the BF1 maps move with the fights and want their
references re-taken when this lands.

Bot-only rounds to an outcome (`round_to_outcome 60000 <seed>`, seeds 1 to 8, all 34 maps,
the rewrite off and on; a bake without the native sides):

| | Rewrite off | Rewrite on |
|---|---|---|
| Rounds finished | 272 of 272 | 272 of 272 |
| The 18 BF2-source maps, 144 rounds | - | every row identical, final hash included |
| The 16 BF1-source maps, mean round length | 219 s | 216 s |
| The 16 BF1-source maps, rounds the first team won | 67 of 128 | 62 of 128 |

So fights on BF1 maps last as long as they did; single maps shift by a few rounds either way
(`bf1_nab2` 0 of 8 to 3 of 8, `bf1_kam1` 6 to 3, `bf1_kas1` 7 to 5, `bf1_nab1` 7 to 5). Bots
against bots cannot show whether a vehicle is too strong against soldiers, since both teams
field them; the stand-in human's win rate is the measure still owed.

## Tests

- `cargo test -p game --lib normalize::` - the rules on in-memory definitions.
- `cargo test -p game --test suite -- normalization::` - the baked classes: equal health and
  damage for equivalent classes and weapons, each table row equal to BF2's own number, pinned
  hits to kill (infantry weapons, and BF1's vehicle and turret guns under rule 5), BF2
  untouched, BF1 changed only inside the rules, a map's own class file following its game,
  the scale keeping every hit count on a trooper, every BF1 soldier and every damage class it
  reaches covered. `OPENBF_CLASSES=<dir>` points them at a bake that carries the native sides.
- `... normalization::print_table --ignored --nocapture` prints the tables below.

## Full tables (generated)

### Soldier health, every BF1 class

| BF1 class | BF1 disc | Now | From |
|---|---|---|---|
| `all_inf_lukeskywalker` | 1000000 | 1000000 | left alone |
| `all_inf_lukeskywalkersnow` | 1000000 | 1000000 | left alone |
| `all_inf_marksman` | 240 | 300 | BF2 `all_inf_sniper` |
| `all_inf_marksmanjungle` | 240 | 300 | BF2 `all_inf_sniper` |
| `all_inf_marksmansnow` | 240 | 300 | BF2 `all_inf_sniper` |
| `all_inf_pilot` | 300 | 400 | BF2 `all_inf_engineer` |
| `all_inf_smuggler` | 450 | 800 | BF2 `all_inf_wookiee` |
| `all_inf_smugglersnow` | 450 | 800 | BF2 `all_inf_wookiee` |
| `all_inf_soldier` | 300 | 400 | BF2 `all_inf_rifleman` |
| `all_inf_soldierdesert` | 300 | 400 | BF2 `all_inf_rifleman` |
| `all_inf_soldierjungle` | 300 | 400 | BF2 `all_inf_rifleman` |
| `all_inf_soldiersnow` | 300 | 400 | BF2 `all_inf_rifleman` |
| `all_inf_soldierurban` | 300 | 400 | BF2 `all_inf_rifleman` |
| `all_inf_vanguard` | 300 | 550 | BF2 `all_inf_rocketeer` |
| `all_inf_vanguardjungle` | 300 | 550 | BF2 `all_inf_rocketeer` |
| `all_inf_vanguardsnow` | 300 | 550 | BF2 `all_inf_rocketeer` |
| `cis_inf_assassindroid` | 240 | 300 | BF2 `all_inf_sniper` |
| `cis_inf_assault` | 300 | 550 | BF2 `all_inf_rocketeer` |
| `cis_inf_battledroid` | 300 | 400 | BF2 `all_inf_rifleman` |
| `cis_inf_battledroid_av` | 300 | 550 | BF2 `all_inf_rocketeer` |
| `cis_inf_battledroid_inf` | 300 | 400 | BF2 `all_inf_rifleman` |
| `cis_inf_battledroid_pilot` | 300 | 400 | BF2 `all_inf_engineer` |
| `cis_inf_battledroid_sniper` | 240 | 300 | BF2 `all_inf_sniper` |
| `cis_inf_countdooku` | 1000000 | 1000000 | left alone |
| `cis_inf_droideka` | 450 | 550 | BF2 `cis_inf_droideka` |
| `cis_inf_pilotdroid` | 300 | 400 | BF2 `all_inf_engineer` |
| `ewk_inf_repair` | 75 | 200 | BF2 `ewk_inf_trooper` |
| `ewk_inf_scout` | 75 | 200 | BF2 `ewk_inf_trooper` |
| `ewk_inf_trooper` | 75 | 200 | BF2 `ewk_inf_trooper` |
| `geo_inf_geonosian` | 125 | 125 | BF2 `geo_inf_geonosian` |
| `gun_inf_defender` | 125 | 200 | BF2 `gun_inf_defender` |
| `gun_inf_rider` | 125 | 200 | BF2 `gun_inf_rider` |
| `gun_inf_soldier` | 125 | 200 | BF2 `gun_inf_soldier` |
| `imp_inf_dark_trooper` | 300 | 300 | BF2 `imp_inf_dark_trooper` |
| `imp_inf_dark_troopersnow` | 300 | 300 | BF2 `imp_inf_dark_trooper` |
| `imp_inf_darthvader` | 1000000 | 1000000 | left alone |
| `imp_inf_pilotatat` | 300 | 400 | BF2 `all_inf_engineer` |
| `imp_inf_pilotatst` | 300 | 400 | BF2 `all_inf_engineer` |
| `imp_inf_pilottie` | 300 | 400 | BF2 `all_inf_engineer` |
| `imp_inf_scout_trooper` | 240 | 300 | BF2 `all_inf_sniper` |
| `imp_inf_scout_troopersnow` | 240 | 300 | BF2 `all_inf_sniper` |
| `imp_inf_shock_trooper` | 300 | 550 | BF2 `all_inf_rocketeer` |
| `imp_inf_shock_troopersnow` | 300 | 550 | BF2 `all_inf_rocketeer` |
| `imp_inf_storm_trooper` | 300 | 400 | BF2 `all_inf_rifleman` |
| `imp_inf_storm_troopersnow` | 300 | 400 | BF2 `all_inf_rifleman` |
| `rep_inf_arc_trooper` | 300 | 550 | BF2 `all_inf_rocketeer` |
| `rep_inf_clone_pilot` | 300 | 400 | BF2 `all_inf_engineer` |
| `rep_inf_clone_sharpshooter` | 240 | 300 | BF2 `all_inf_sniper` |
| `rep_inf_clone_trooper` | 300 | 400 | BF2 `all_inf_rifleman` |
| `rep_inf_jet_trooper` | 240 | 300 | BF2 `rep_inf_ep3_jettrooper` |
| `rep_inf_macewindu` | 1000000 | 1000000 | left alone |
| `tat_inf_jawa` | 200 | 250 | BF2 `tat_inf_jawa` |
| `tat_inf_tuskenhunter` | 200 | 300 | BF2 `tat_inf_tuskenhunter` |
| `tat_inf_tuskenraider` | 300 | 400 | BF2 `tat_inf_tuskenraider` |
| `wok_inf_mechanic` | 300 | 300 | BF2 `wok_inf_warrior` |
| `wok_inf_rocketeer` | 300 | 300 | BF2 `wok_inf_warrior` |
| `wok_inf_warrior` | 300 | 300 | BF2 `wok_inf_warrior` |

### BF1 infantry damage classes

| BF1 class | To a person, disc | Hits on 300 | Now | Hits on 400 | Factor | From |
|---|---|---|---|---|---|---|
| `all_weap_inf_bowcaster_ord` | 60 | 5 | 80 | 5 | 1.333 | x4/3 |
| `all_weap_inf_bowcaster_ord_high` | 75 | 4 | 100 | 4 | 1.333 | x4/3 |
| `all_weap_inf_crackgrenade_exp` | 100 | 3 | 133.33 | 3 | 1.333 | x4/3 |
| `all_weap_inf_mortar_launcher_exp` | 320 | 1 | 400 | 1 | 1.250 | BF2 `all_weap_inf_mortar_launcher_exp` |
| `all_weap_inf_orbital_attack_salvo_exp` | 500 | 1 | 666.67 | 1 | 1.333 | x4/3 |
| `all_weap_inf_orbital_attack_salvo_ord` | 1000 | 1 | 1333.33 | 1 | 1.333 | x4/3 |
| `all_weap_inf_pistol_ord` | 40 | 8 | 60 | 7 | 1.500 | BF2 `all_weap_inf_pistol_ord` |
| `all_weap_inf_remote_charge_dispenser_exp` | 2000 | 1 | 2666.67 | 1 | 1.333 | x4/3 |
| `all_weap_inf_remotedroid_blaster_ord` | 10 | 30 | 60 | 7 | 6.000 | BF2 `all_weap_inf_remotedroid_blaster_ord` |
| `all_weap_inf_remotedroid_exp2` | 50 | 6 | 66.67 | 6 | 1.333 | x4/3 |
| `all_weap_inf_rifle_ord` | 60 | 5 | 80 | 5 | 1.333 | x4/3 |
| `all_weap_inf_shotgun_ord` | 65 | 5 | 86.67 | 5 | 1.333 | x4/3 |
| `all_weap_inf_sniperrifle_ord` | 160 | 2 | 300 | 2 | 1.875 | BF2 `all_weap_inf_sniper_rifle_ord` |
| `all_weap_inf_thermaldetonator_exp` | 320 | 1 | 450 | 1 | 1.406 | BF2 `all_weap_inf_thermaldetonator_exp` |
| `all_weap_inf_torpedo_launcher_exp` | 320 | 1 | 400 | 1 | 1.250 | BF2 `all_weap_inf_rocket_launcher_exp` |
| `all_weap_inf_torpedo_launcher_ord` | 600 | 1 | 800 | 1 | 1.333 | x4/3 |
| `cis_weap_inf_crackgrenade_exp` | 100 | 3 | 133.33 | 3 | 1.333 | x4/3 |
| `cis_weap_inf_mortar_launcher_exp` | 300 | 1 | 400 | 1 | 1.333 | BF2 `all_weap_inf_mortar_launcher_exp` |
| `cis_weap_inf_mortar_launcher_ord1` | 150 | 2 | 200 | 2 | 1.333 | x4/3 |
| `cis_weap_inf_orbital_attack_salvo_exp` | 500 | 1 | 666.67 | 1 | 1.333 | x4/3 |
| `cis_weap_inf_orbital_attack_salvo_ord` | 1000 | 1 | 1333.33 | 1 | 1.333 | x4/3 |
| `cis_weap_inf_pistol_ord` | 40 | 8 | 60 | 7 | 1.500 | BF2 `all_weap_inf_pistol_ord` |
| `cis_weap_inf_remotedroid_blaster_ord` | 10 | 30 | 60 | 7 | 6.000 | BF2 `all_weap_inf_remotedroid_blaster_ord` |
| `cis_weap_inf_remotedroid_exp2` | 50 | 6 | 66.67 | 6 | 1.333 | x4/3 |
| `cis_weap_inf_repeater_ord` | 55 | 6 | 73.33 | 6 | 1.333 | x4/3 |
| `cis_weap_inf_rifle_ord` | 60 | 5 | 80 | 5 | 1.333 | x4/3 |
| `cis_weap_inf_sniperrifle_ord` | 160 | 2 | 300 | 2 | 1.875 | BF2 `all_weap_inf_sniper_rifle_ord` |
| `cis_weap_inf_thermaldetonator_exp` | 320 | 1 | 450 | 1 | 1.406 | BF2 `all_weap_inf_thermaldetonator_exp` |
| `cis_weap_inf_torpedo_launcher_exp` | 320 | 1 | 400 | 1 | 1.250 | BF2 `all_weap_inf_rocket_launcher_exp` |
| `cis_weap_inf_torpedo_launcher_ord` | 600 | 1 | 800 | 1 | 1.333 | x4/3 |
| `cis_weap_inf_wrist_blaster_ord` | 60 | 5 | 80 | 5 | 1.333 | x4/3 |
| `cis_weap_inf_wrist_rocket_exp` | 100 | 3 | 133.33 | 3 | 1.333 | x4/3 |
| `cis_weap_inf_wrist_rocket_ord` | 200 | 2 | 266.67 | 2 | 1.333 | x4/3 |
| `com_weap_inf_empmine_exp` | 100 | 3 | 100 | 4 | 1.000 | BF2 `com_weap_inf_empmine_exp` |
| `com_weap_inf_landmine_exp` | 1750 | 1 | 2333.33 | 1 | 1.333 | x4/3 |
| `com_weap_inf_seismic_mine_exp` | 5000 | 1 | 5000 | 1 | 1.000 | BF2 `com_weap_inf_seismic_mine_exp` |
| `ewk_weap_inf_rock_ord` | 50 | 6 | 66.67 | 6 | 1.333 | x4/3 |
| `ewk_weap_inf_spear_ord` | 150 | 2 | 200 | 2 | 1.333 | BF2 `ewk_weap_inf_spear_ord` |
| `geo_weap_inf_blunderbuss_ord` | 25 | 12 | 100 | 4 | 4.000 | BF2 `geo_weap_inf_blunderbuss_hidden` |
| `gun_weap_inf_haywiredetonator_exp` | 300 | 1 | 400 | 1 | 1.333 | x4/3 |
| `imp_weap_inf_crackgrenade_exp` | 100 | 3 | 133.33 | 3 | 1.333 | x4/3 |
| `imp_weap_inf_mortar_launcher_exp` | 320 | 1 | 400 | 1 | 1.250 | BF2 `all_weap_inf_mortar_launcher_exp` |
| `imp_weap_inf_mortar_launcher_ord1` | 150 | 2 | 200 | 2 | 1.333 | x4/3 |
| `imp_weap_inf_orbital_attack_salvo_exp` | 500 | 1 | 666.67 | 1 | 1.333 | x4/3 |
| `imp_weap_inf_orbital_attack_salvo_ord` | 1000 | 1 | 1333.33 | 1 | 1.333 | x4/3 |
| `imp_weap_inf_pistol_ord` | 40 | 8 | 60 | 7 | 1.500 | BF2 `all_weap_inf_pistol_ord` |
| `imp_weap_inf_remotedroid_blaster_ord` | 10 | 30 | 60 | 7 | 6.000 | BF2 `all_weap_inf_remotedroid_blaster_ord` |
| `imp_weap_inf_remotedroid_exp2` | 50 | 6 | 66.67 | 6 | 1.333 | x4/3 |
| `imp_weap_inf_rifle_ord` | 60 | 5 | 70 | 6 | 1.167 | BF2 `imp_weap_inf_rifle_ord` |
| `imp_weap_inf_shotgun_ord` | 65 | 5 | 86.67 | 5 | 1.333 | x4/3 |
| `imp_weap_inf_sniperrifle_ord` | 160 | 2 | 300 | 2 | 1.875 | BF2 `all_weap_inf_sniper_rifle_ord` |
| `imp_weap_inf_thermaldetonator_exp` | 320 | 1 | 450 | 1 | 1.406 | BF2 `all_weap_inf_thermaldetonator_exp` |
| `imp_weap_inf_torpedo_launcher_exp` | 320 | 1 | 400 | 1 | 1.250 | BF2 `all_weap_inf_rocket_launcher_exp` |
| `imp_weap_inf_torpedo_launcher_ord` | 600 | 1 | 800 | 1 | 1.333 | x4/3 |
| `rep_weap_inf_arccaster_ord` | 250 | 2 | 333.33 | 2 | 1.333 | x4/3 |
| `rep_weap_inf_commando_pistol_ord` | 40 | 8 | 60 | 7 | 1.500 | BF2 `rep_weap_inf_commando_pistol_ord` |
| `rep_weap_inf_crackgrenade_exp` | 100 | 3 | 133.33 | 3 | 1.333 | x4/3 |
| `rep_weap_inf_haywiredetonator_exp` | 225 | 2 | 300 | 2 | 1.333 | x4/3 |
| `rep_weap_inf_orbital_attack_salvo_exp` | 500 | 1 | 666.67 | 1 | 1.333 | x4/3 |
| `rep_weap_inf_orbital_attack_salvo_ord` | 1000 | 1 | 1333.33 | 1 | 1.333 | x4/3 |
| `rep_weap_inf_pistol_ord` | 40 | 8 | 60 | 7 | 1.500 | BF2 `all_weap_inf_pistol_ord` |
| `rep_weap_inf_remotedroid_blaster_ord` | 10 | 30 | 60 | 7 | 6.000 | BF2 `all_weap_inf_remotedroid_blaster_ord` |
| `rep_weap_inf_remotedroid_exp2` | 50 | 6 | 66.67 | 6 | 1.333 | x4/3 |
| `rep_weap_inf_rifle_ord` | 60 | 5 | 80 | 5 | 1.333 | x4/3 |
| `rep_weap_inf_rocket_launcher_exp` | 150 | 2 | 200 | 2 | 1.333 | x4/3 |
| `rep_weap_inf_rocket_launcher_ord` | 60 | 5 | 80 | 5 | 1.333 | x4/3 |
| `rep_weap_inf_sniperrifle_ord` | 160 | 2 | 300 | 2 | 1.875 | BF2 `all_weap_inf_sniper_rifle_ord` |
| `rep_weap_inf_thermaldetonator_exp` | 320 | 1 | 450 | 1 | 1.406 | BF2 `all_weap_inf_thermaldetonator_exp` |
| `rep_weap_inf_torpedo_launcher_exp` | 320 | 1 | 400 | 1 | 1.250 | BF2 `all_weap_inf_rocket_launcher_exp` |
| `rep_weap_inf_torpedo_launcher_ord` | 600 | 1 | 800 | 1 | 1.333 | x4/3 |
| `tat_weap_inf_tuskenrifle_ord` | 40 | 8 | 80 | 5 | 2.000 | BF2 `tat_weap_inf_tuskenrifle_ord` |
| `tat_weap_inf_tuskensniperrifle_ord` | 160 | 2 | 200 | 2 | 1.250 | BF2 `tat_weap_inf_tuskensniperrifle_ord` |
| `wok_weap_inf_bowcaster_ord` | 60 | 5 | 80 | 5 | 1.333 | x4/3 |
| `wok_weap_inf_bowcaster_ord_high` | 75 | 4 | 100 | 4 | 1.333 | x4/3 |
| `wok_weap_inf_torpedo_launcher_exp` | 400 | 1 | 400 | 1 | 1.000 | BF2 `all_weap_inf_rocket_launcher_exp` |
| `wok_weap_inf_torpedo_launcher_ord` | 600 | 1 | 800 | 1 | 1.333 | x4/3 |

### BF1 vehicle, turret and blast classes (interim x4/3)

| BF1 class | From | To a person, disc | Hits on 300 | Now | Hits on 400 |
|---|---|---|---|---|---|
| `all_droid_R2D2_exp` | sides | 20 | 15 | 26.67 | 15 |
| `all_fly_snowspeeder_exp` | sides | 32 | 10 | 42.67 | 10 |
| `all_fly_ywing_exp` | sides | 48 | 7 | 64 | 7 |
| `all_hover_combatspeeder_exp` | sides | 100 | 3 | 133.33 | 3 |
| `all_hover_hovernaut_exp` | sides | 50 | 6 | 66.67 | 6 |
| `all_weap_fly_kwing_bomb_exp` | sides | 750 | 1 | 1000 | 1 |
| `all_weap_fly_kwing_bomb_ord` | sides | 60 | 5 | 80 | 5 |
| `all_weap_fly_kwing_conc_exp` | sides | 50 | 6 | 66.67 | 6 |
| `all_weap_fly_kwing_conc_ord` | sides | 200 | 2 | 266.67 | 2 |
| `all_weap_fly_kwing_laser_exp` | sides | 100 | 3 | 133.33 | 3 |
| `all_weap_fly_kwing_laser_ord` | sides | 250 | 2 | 333.33 | 2 |
| `all_weap_fly_kwing_miss_exp` | sides | 100 | 3 | 133.33 | 3 |
| `all_weap_fly_kwing_miss_ord` | sides | 150 | 2 | 200 | 2 |
| `all_weap_fly_snowspeeder_cannon_exp` | sides | 100 | 3 | 133.33 | 3 |
| `all_weap_fly_snowspeeder_cannon_ord` | sides | 400 | 1 | 533.33 | 1 |
| `all_weap_fly_snowspeeder_reargun_ord` | sides | 50 | 6 | 66.67 | 6 |
| `all_weap_fly_xwing_cannon_exp` | sides | 100 | 3 | 133.33 | 3 |
| `all_weap_fly_xwing_cannon_ord` | sides | 600 | 1 | 800 | 1 |
| `all_weap_fly_xwing_ptorpedo_exp` | sides | 100 | 3 | 133.33 | 3 |
| `all_weap_fly_xwing_ptorpedo_ord` | sides | 1500 | 1 | 2000 | 1 |
| `all_weap_fly_ywing_bomb_exp` | sides | 750 | 1 | 1000 | 1 |
| `all_weap_fly_ywing_bomb_ord` | sides | 600 | 1 | 800 | 1 |
| `all_weap_fly_ywing_cannon_exp` | sides | 100 | 3 | 133.33 | 3 |
| `all_weap_fly_ywing_cannon_ord` | sides | 600 | 1 | 800 | 1 |
| `all_weap_hover_combatspeeder_cannon1_exp` | sides | 300 | 1 | 400 | 1 |
| `all_weap_hover_combatspeeder_cannon1_ord` | sides | 300 | 1 | 400 | 1 |
| `all_weap_hover_combatspeeder_cannon_exp` | sides | 400 | 1 | 533.33 | 1 |
| `all_weap_hover_combatspeeder_cannon_ord` | sides | 800 | 1 | 1066.67 | 1 |
| `all_weap_hover_combatspeeder_missile_exp` | sides | 300 | 1 | 400 | 1 |
| `all_weap_hover_combatspeeder_missile_ord` | sides | 400 | 1 | 533.33 | 1 |
| `all_weap_hover_hovernaut_blaster_exp` | sides | 200 | 2 | 266.67 | 2 |
| `all_weap_hover_hovernaut_blaster_ord` | sides | 450 | 1 | 600 | 1 |
| `all_weap_hover_hovernaut_cannon1_ord` | sides | 1200 | 1 | 1600 | 1 |
| `all_weap_hover_hovernaut_missile_exp` | sides | 100 | 3 | 133.33 | 3 |
| `all_weap_hover_hovernaut_missile_ord` | sides | 120 | 3 | 160 | 3 |
| `cis_fly_droidfighter_exp` | sides | 32 | 10 | 42.67 | 10 |
| `cis_fly_geofighter_exp` | sides | 32 | 10 | 42.67 | 10 |
| `cis_hover_aat_exp` | sides | 100 | 3 | 133.33 | 3 |
| `cis_hover_mtt_exp` | sides | 50 | 6 | 66.67 | 6 |
| `cis_hover_stap_exp` | sides | 20 | 15 | 26.67 | 15 |
| `cis_tread_hailfire_exp` | sides | 32 | 10 | 42.67 | 10 |
| `cis_walk_spider_exp` | sides | 32 | 10 | 42.67 | 10 |
| `cis_weap_fly_droidfighter_cannon_exp` | sides | 300 | 1 | 400 | 1 |
| `cis_weap_fly_droidfighter_cannon_ord` | sides | 800 | 1 | 1066.67 | 1 |
| `cis_weap_fly_geofighter_cannon_exp` | sides | 300 | 1 | 400 | 1 |
| `cis_weap_fly_geofighter_cannon_ord` | sides | 300 | 1 | 400 | 1 |
| `cis_weap_fly_maf_cannon_exp` | sides | 300 | 1 | 400 | 1 |
| `cis_weap_fly_maf_cannon_ord` | sides | 300 | 1 | 400 | 1 |
| `cis_weap_fly_maf_laser_ord` | sides | 1000 | 1 | 1333.33 | 1 |
| `cis_weap_fly_maf_torpedo_exp` | sides | 300 | 1 | 400 | 1 |
| `cis_weap_fly_maf_torpedo_ord` | sides | 800 | 1 | 1066.67 | 1 |
| `cis_weap_hover_aat_cannon_exp` | sides | 300 | 1 | 400 | 1 |
| `cis_weap_hover_aat_cannon_ord` | sides | 400 | 1 | 533.33 | 1 |
| `cis_weap_hover_aat_launcher_exp` | sides | 400 | 1 | 533.33 | 1 |
| `cis_weap_hover_aat_launcher_ord` | sides | 1200 | 1 | 1600 | 1 |
| `cis_weap_hover_stap_cannon_exp` | sides | 20 | 15 | 26.67 | 15 |
| `cis_weap_hover_stap_cannon_ord` | sides | 300 | 1 | 400 | 1 |
| `cis_weap_tread_hailfire_missile_exp` | sides | 400 | 1 | 533.33 | 1 |
| `cis_weap_tread_hailfire_missile_ord` | sides | 300 | 1 | 400 | 1 |
| `cis_weap_walk_spider_cannon_exp` | sides | 400 | 1 | 533.33 | 1 |
| `cis_weap_walk_spider_cannon_ord` | sides | 400 | 1 | 533.33 | 1 |
| `cis_weap_walk_spider_laser_ord` | sides | 1000 | 1 | 1333.33 | 1 |
| `com_inf_rechargedroid_exp` | sides | 20 | 15 | 26.67 | 15 |
| `com_weap_bldg_gunturret_exp` | sides | 25 | 12 | 33.33 | 12 |
| `com_weap_bldg_gunturret_ord` | sides | 100 | 3 | 133.33 | 3 |
| `imp_droid_probe_exp` | sides | 20 | 15 | 26.67 | 15 |
| `imp_fly_tiebomber_exp` | sides | 32 | 10 | 42.67 | 10 |
| `imp_fly_tiefighter_exp` | sides | 32 | 10 | 42.67 | 10 |
| `imp_fly_trooptrans_exp` | sides | 32 | 10 | 42.67 | 10 |
| `imp_hover_fightertank_exp` | sides | 100 | 3 | 133.33 | 3 |
| `imp_hover_speederbike_exp` | sides | 20 | 15 | 26.67 | 15 |
| `imp_walk_atat_exp` | sides | 400 | 1 | 533.33 | 1 |
| `imp_walk_atat_finalexp` | sides | 50 | 6 | 66.67 | 6 |
| `imp_walk_atst_exp` | sides | 50 | 6 | 66.67 | 6 |
| `imp_walk_atst_finalexp` | sides | 50 | 6 | 66.67 | 6 |
| `imp_weap_droid_probe_blaster_ord` | sides | 75 | 4 | 100 | 4 |
| `imp_weap_fly_tiebomber_bomb_exp` | sides | 750 | 1 | 1000 | 1 |
| `imp_weap_fly_tiebomber_bomb_ord` | sides | 600 | 1 | 800 | 1 |
| `imp_weap_fly_tiebomber_ptorpedo_ord` | sides | 1200 | 1 | 1600 | 1 |
| `imp_weap_fly_tiefighter_cannon_exp` | sides | 100 | 3 | 133.33 | 3 |
| `imp_weap_fly_tiefighter_cannon_ord` | sides | 800 | 1 | 1066.67 | 1 |
| `imp_weap_hover_fightertank_cannon_exp` | sides | 300 | 1 | 400 | 1 |
| `imp_weap_hover_fightertank_cannon_ord` | sides | 300 | 1 | 400 | 1 |
| `imp_weap_hover_fightertank_laser_ord` | sides | 800 | 1 | 1066.67 | 1 |
| `imp_weap_hover_fightertank_missile_exp` | sides | 300 | 1 | 400 | 1 |
| `imp_weap_hover_fightertank_missile_ord` | sides | 400 | 1 | 533.33 | 1 |
| `imp_weap_hover_speederbike_cannon_ord` | sides | 300 | 1 | 400 | 1 |
| `imp_weap_walk_atat_cannon_chin_exp4` | sides | 800 | 1 | 1066.67 | 1 |
| `imp_weap_walk_atat_cannon_chin_ord1` | sides | 2500 | 1 | 3333.33 | 1 |
| `imp_weap_walk_atat_cannon_head_exp` | sides | 400 | 1 | 533.33 | 1 |
| `imp_weap_walk_atat_cannon_head_ord` | sides | 400 | 1 | 533.33 | 1 |
| `imp_weap_walk_atst_cannon_chin1_exp` | sides | 400 | 1 | 533.33 | 1 |
| `imp_weap_walk_atst_cannon_chin1_ord` | sides | 450 | 1 | 600 | 1 |
| `imp_weap_walk_atst_cannon_head_exp` | sides | 400 | 1 | 533.33 | 1 |
| `imp_weap_walk_atst_cannon_head_ord` | sides | 400 | 1 | 533.33 | 1 |
| `rep_bldg_forwardcenter_exp` | sides | 32 | 10 | 42.67 | 10 |
| `rep_fly_gunship_exp` | sides | 32 | 10 | 42.67 | 10 |
| `rep_fly_jedifighter_exp` | sides | 32 | 10 | 42.67 | 10 |
| `rep_hover_fightertank_exp` | sides | 400 | 1 | 533.33 | 1 |
| `rep_hover_speederbike_exp` | sides | 20 | 15 | 26.67 | 15 |
| `rep_walk_atte_exp` | sides | 32 | 10 | 42.67 | 10 |
| `rep_weap_fly_gunship_ball_ord` | sides | 600 | 1 | 800 | 1 |
| `rep_weap_fly_gunship_cannon_exp` | sides | 300 | 1 | 400 | 1 |
| `rep_weap_fly_gunship_cannon_ord` | sides | 300 | 1 | 400 | 1 |
| `rep_weap_fly_gunship_missile_exp` | sides | 300 | 1 | 400 | 1 |
| `rep_weap_fly_gunship_missile_ord` | sides | 400 | 1 | 533.33 | 1 |
| `rep_weap_fly_gunship_wing_ord` | sides | 450 | 1 | 600 | 1 |
| `rep_weap_fly_jedifighter_cannon_exp` | sides | 300 | 1 | 400 | 1 |
| `rep_weap_fly_jedifighter_cannon_ord` | sides | 400 | 1 | 533.33 | 1 |
| `rep_weap_fly_jedifighter_missile_exp` | sides | 300 | 1 | 400 | 1 |
| `rep_weap_fly_jedifighter_missile_ord` | sides | 1000 | 1 | 1333.33 | 1 |
| `rep_weap_hover_fightertank_cannon_exp` | sides | 400 | 1 | 533.33 | 1 |
| `rep_weap_hover_fightertank_cannon_ord` | sides | 300 | 1 | 400 | 1 |
| `rep_weap_hover_fightertank_laser_ord` | sides | 800 | 1 | 1066.67 | 1 |
| `rep_weap_hover_fightertank_missile_exp` | sides | 300 | 1 | 400 | 1 |
| `rep_weap_hover_fightertank_missile_ord` | sides | 400 | 1 | 533.33 | 1 |
| `rep_weap_hover_speederbike_cannon_exp` | sides | 100 | 3 | 133.33 | 3 |
| `rep_weap_hover_speederbike_cannon_ord` | sides | 50 | 6 | 66.67 | 6 |
| `rep_weap_walk_atte_cannon_exp` | sides | 400 | 1 | 533.33 | 1 |
| `rep_weap_walk_atte_cannon_heavy_exp` | sides | 400 | 1 | 533.33 | 1 |
| `rep_weap_walk_atte_cannon_heavy_ord` | sides | 400 | 1 | 533.33 | 1 |
| `rep_weap_walk_atte_cannon_ord` | sides | 400 | 1 | 533.33 | 1 |
| `rep_weap_walk_atte_turretcannon_exp4` | sides | 400 | 1 | 533.33 | 1 |
| `rep_weap_walk_atte_turretcannon_ord1` | sides | 1200 | 1 | 1600 | 1 |
| `bes_bldg_AAA_turret_exp` | bf1_bes1 | 32 | 10 | 42.67 | 10 |
| `bes_fly_cloudcar_exp` | bf1_bes1 | 32 | 10 | 42.67 | 10 |
| `bes_weap_cloudcar_miss_exp` | bf1_bes1 | 100 | 3 | 133.33 | 3 |
| `bes_weap_cloudcar_miss_ord` | bf1_bes1 | 50 | 6 | 66.67 | 6 |
| `bes_weap_fly_cloudcar_cannon_exp` | bf1_bes1 | 100 | 3 | 133.33 | 3 |
| `bes_weap_fly_cloudcar_cannon_ord` | bf1_bes1 | 200 | 2 | 266.67 | 2 |
| `kam_weap_bldg_AAA_turret_cannon_exp` | bf1_bes1 | 300 | 1 | 400 | 1 |
| `kam_weap_bldg_AAA_turret_cannon_ord` | bf1_bes1 | 800 | 1 | 1066.67 | 1 |
| `geo_bldg_technounion_exp` | bf1_geo1 | 50 | 6 | 66.67 | 6 |
| `geo_weap_bldg_geoturret_ord` | bf1_geo1 | 400 | 1 | 533.33 | 1 |
| `hoth_bldg_dish_turret_exp` | bf1_hot1 | 50 | 6 | 66.67 | 6 |
| `hoth_bldg_hoth_turret_exp` | bf1_hot1 | 50 | 6 | 66.67 | 6 |
| `hoth_weap_bldg_dishturret_cannon_exp1` | bf1_hot1 | 50 | 6 | 66.67 | 6 |
| `hoth_weap_bldg_dishturret_cannon_exp2` | bf1_hot1 | 100 | 3 | 133.33 | 3 |
| `hoth_weap_bldg_dishturret_cannon_exp3` | bf1_hot1 | 200 | 2 | 266.67 | 2 |
| `hoth_weap_bldg_dishturret_cannon_exp4` | bf1_hot1 | 300 | 1 | 400 | 1 |
| `hoth_weap_bldg_dishturret_cannon_ord` | bf1_hot1 | 200 | 2 | 266.67 | 2 |
| `hoth_weap_bldg_hothturret_cannon_ord` | bf1_hot1 | 300 | 1 | 400 | 1 |
| `yav_bldg_tower_turret_exp` | bf1_yav1 | 50 | 6 | 66.67 | 6 |
| `yav_weap_bldg_tower_minigun_ord` | bf1_yav1 | 100 | 3 | 133.33 | 3 |
| `tat_hover_skiff_exp` | tat1 | 20 | 15 | 26.67 | 15 |

### BF2 infantry damage classes (the reference, unchanged)

| BF2 class | To a person | Hits on 400 |
|---|---|---|
| `all_weap_award_pistol_ord` | 150 | 3 |
| `all_weap_award_rifle_ord` | 150 | 3 |
| `all_weap_award_shotgun_ord` | 85 | 5 |
| `all_weap_award_sniper_rifle_ord` | 600 | 1 |
| `all_weap_inf_autoturret_ord` | 100 | 4 |
| `all_weap_inf_bowcaster_ord` | 75 | 6 |
| `all_weap_inf_bowcaster_ord_high` | 300 | 2 |
| `all_weap_inf_commando_pistol_ord` | 60 | 7 |
| `all_weap_inf_detpack_exp` | 1200 | 1 |
| `all_weap_inf_incinerator_exp` | 10 | 40 |
| `all_weap_inf_incinerator_ord` | 30 | 14 |
| `all_weap_inf_mortar_launcher_exp` | 400 | 1 |
| `all_weap_inf_mortar_launcher_ord` | 400 | 1 |
| `all_weap_inf_mortar_launcher_ord1` | 100 | 4 |
| `all_weap_inf_pistol_ord` | 60 | 7 |
| `all_weap_inf_remotedroid_blaster_ord` | 60 | 7 |
| `all_weap_inf_rifle_ord` | 80 | 5 |
| `all_weap_inf_rocket_launcher_exp` | 400 | 1 |
| `all_weap_inf_rocket_launcher_ord` | 500 | 1 |
| `all_weap_inf_shotgun_ord` | 65 | 7 |
| `all_weap_inf_sniper_rifle_ord` | 300 | 2 |
| `all_weap_inf_thermaldetonator_exp` | 450 | 1 |
| `all_weap_inf_timebomb_exp` | 5000 | 1 |
| `cis_weap_award_pistol_ord` | 150 | 3 |
| `cis_weap_award_rifle_ord` | 150 | 3 |
| `cis_weap_award_shotgun_ord` | 85 | 5 |
| `cis_weap_award_sniper_rifle_ord` | 600 | 1 |
| `cis_weap_award_wrist_blaster_ord` | 150 | 3 |
| `cis_weap_inf_autoturret_ord` | 100 | 4 |
| `cis_weap_inf_bulldog_exp` | 150 | 3 |
| `cis_weap_inf_bulldog_ord` | 100 | 4 |
| `cis_weap_inf_detpack_exp` | 1200 | 1 |
| `cis_weap_inf_mortar_launcher_exp` | 400 | 1 |
| `cis_weap_inf_mortar_launcher_ord` | 400 | 1 |
| `cis_weap_inf_mortar_launcher_ord1` | 100 | 4 |
| `cis_weap_inf_pistol_ord` | 60 | 7 |
| `cis_weap_inf_remotedroid_blaster_ord` | 60 | 7 |
| `cis_weap_inf_repeater_ord` | 80 | 5 |
| `cis_weap_inf_rifle_ord` | 80 | 5 |
| `cis_weap_inf_rocket_launcher_exp` | 400 | 1 |
| `cis_weap_inf_rocket_launcher_ord` | 500 | 1 |
| `cis_weap_inf_shotgun_ord` | 65 | 7 |
| `cis_weap_inf_sniper_rifle_ord` | 300 | 2 |
| `cis_weap_inf_thermaldetonator_exp` | 450 | 1 |
| `cis_weap_inf_timebomb_exp` | 5000 | 1 |
| `cis_weap_inf_wrist_blaster_ord` | 100 | 4 |
| `cis_weap_inf_wrist_rocket_exp` | 250 | 2 |
| `cis_weap_inf_wrist_rocket_ord` | 100 | 4 |
| `com_weap_award_pistol_ord` | 150 | 3 |
| `com_weap_award_rifle_ord` | 150 | 3 |
| `com_weap_award_shotgun_ord` | 85 | 5 |
| `com_weap_award_sniper_rifle_ord` | 600 | 1 |
| `com_weap_inf_arccaster_exp` | 7.5 | 54 |
| `com_weap_inf_arccaster_ord` | 325 | 2 |
| `com_weap_inf_autoturret_ord` | 100 | 4 |
| `com_weap_inf_bowcaster_ord` | 75 | 6 |
| `com_weap_inf_bowcaster_ord_high` | 300 | 2 |
| `com_weap_inf_bulldog_exp` | 150 | 3 |
| `com_weap_inf_bulldog_ord` | 100 | 4 |
| `com_weap_inf_chaingun_ord` | 25 | 16 |
| `com_weap_inf_commando_pistol_ord` | 60 | 7 |
| `com_weap_inf_defenseturret_ord` | 100 | 4 |
| `com_weap_inf_detpack_exp` | 1200 | 1 |
| `com_weap_inf_empmine_exp` | 100 | 4 |
| `com_weap_inf_flamethrower_ord` | 15 | 27 |
| `com_weap_inf_force_lightning_exp` | 7.5 | 54 |
| `com_weap_inf_force_lightning_ord` | 150 | 3 |
| `com_weap_inf_force_pull_exp` | 0.5 | 800 |
| `com_weap_inf_force_push_exp` | 0.5 | 800 |
| `com_weap_inf_incinerator_exp` | 10 | 40 |
| `com_weap_inf_incinerator_ord` | 30 | 14 |
| `com_weap_inf_landmine_exp` | 8000 | 1 |
| `com_weap_inf_mortar_launcher_exp` | 400 | 1 |
| `com_weap_inf_mortar_launcher_ord` | 400 | 1 |
| `com_weap_inf_mortar_launcher_ord1` | 100 | 4 |
| `com_weap_inf_pistol_ord` | 60 | 7 |
| `com_weap_inf_remotedroid_blaster_ord` | 60 | 7 |
| `com_weap_inf_rifle_ord` | 80 | 5 |
| `com_weap_inf_rocket_launcher_exp` | 400 | 1 |
| `com_weap_inf_rocket_launcher_ord` | 500 | 1 |
| `com_weap_inf_sabre_throw_ord` | 400 | 1 |
| `com_weap_inf_seismic_mine_exp` | 5000 | 1 |
| `com_weap_inf_shotgun_ord` | 65 | 7 |
| `com_weap_inf_sniper_rifle_ord` | 300 | 2 |
| `com_weap_inf_sonic_blaster_hidden` | 100 | 4 |
| `com_weap_inf_sonic_blaster_ord` | 100 | 4 |
| `com_weap_inf_thermaldetonator_exp` | 450 | 1 |
| `com_weap_inf_timebomb_exp` | 5000 | 1 |
| `com_weap_inf_wrist_rocket_exp` | 250 | 2 |
| `com_weap_inf_wrist_rocket_ord` | 100 | 4 |
| `dlch_com_weap_inf_force_pull_exp` | 0.5 | 800 |
| `dlch_com_weap_inf_force_push_exp` | 0.5 | 800 |
| `dlch_com_weap_inf_sabre_throw_ord` | 400 | 1 |
| `dlch_com_weap_inf_wrist_rocket_ord` | 100 | 4 |
| `ewk_weap_inf_rock_exp` | 25 | 16 |
| `ewk_weap_inf_rock_ord` | 50 | 8 |
| `ewk_weap_inf_spear_ord` | 200 | 2 |
| `geo_weap_inf_blunderbuss_hidden` | 100 | 4 |
| `geo_weap_inf_blunderbuss_ord` | 100 | 4 |
| `geo_weap_inf_bulldog_exp` | 150 | 3 |
| `geo_weap_inf_bulldog_ord` | 100 | 4 |
| `imp_weap_award_pistol_ord` | 150 | 3 |
| `imp_weap_award_rifle_ord` | 150 | 3 |
| `imp_weap_award_shotgun_ord` | 85 | 5 |
| `imp_weap_award_sniper_rifle_ord` | 600 | 1 |
| `imp_weap_inf_arccaster_exp` | 7.5 | 54 |
| `imp_weap_inf_arccaster_ord` | 325 | 2 |
| `imp_weap_inf_autoturret_ord` | 100 | 4 |
| `imp_weap_inf_commando_pistol_ord` | 60 | 7 |
| `imp_weap_inf_detpack_exp` | 1200 | 1 |
| `imp_weap_inf_mortar_launcher_exp` | 400 | 1 |
| `imp_weap_inf_mortar_launcher_ord` | 400 | 1 |
| `imp_weap_inf_mortar_launcher_ord1` | 100 | 4 |
| `imp_weap_inf_pistol_ord` | 60 | 7 |
| `imp_weap_inf_remotedroid_blaster_ord` | 60 | 7 |
| `imp_weap_inf_rifle_ord` | 70 | 6 |
| `imp_weap_inf_rocket_launcher_exp` | 400 | 1 |
| `imp_weap_inf_rocket_launcher_ord` | 500 | 1 |
| `imp_weap_inf_shotgun_ord` | 65 | 7 |
| `imp_weap_inf_sniper_rifle_ord` | 300 | 2 |
| `imp_weap_inf_sonic_blaster_hidden` | 100 | 4 |
| `imp_weap_inf_sonic_blaster_ord` | 100 | 4 |
| `imp_weap_inf_thermaldetonator_exp` | 450 | 1 |
| `imp_weap_inf_timebomb_exp` | 5000 | 1 |
| `imp_weap_inf_wrist_rocket_exp` | 250 | 2 |
| `imp_weap_inf_wrist_rocket_ord` | 100 | 4 |
| `rep_weap_award_pistol_ord` | 150 | 3 |
| `rep_weap_award_rifle_ord` | 150 | 3 |
| `rep_weap_award_shotgun_ord` | 85 | 5 |
| `rep_weap_award_sniper_rifle_ord` | 600 | 1 |
| `rep_weap_inf_autoturret_ord` | 100 | 4 |
| `rep_weap_inf_chaingun_ord` | 25 | 16 |
| `rep_weap_inf_commando_pistol_ord` | 60 | 7 |
| `rep_weap_inf_detpack_exp` | 1200 | 1 |
| `rep_weap_inf_emp_launcher_ord` | 200 | 2 |
| `rep_weap_inf_pistol_ord` | 60 | 7 |
| `rep_weap_inf_remotedroid_blaster_ord` | 60 | 7 |
| `rep_weap_inf_rifle_ord` | 80 | 5 |
| `rep_weap_inf_rocket_launcher_exp` | 400 | 1 |
| `rep_weap_inf_rocket_launcher_ord` | 500 | 1 |
| `rep_weap_inf_shotgun_ord` | 65 | 7 |
| `rep_weap_inf_sniper_rifle_ord` | 300 | 2 |
| `rep_weap_inf_thermaldetonator_exp` | 450 | 1 |
| `rep_weap_inf_timebomb_exp` | 5000 | 1 |
| `tat_weap_inf_jawa_arccaster_exp` | 7.5 | 54 |
| `tat_weap_inf_jawa_arccaster_ord` | 165 | 3 |
| `tat_weap_inf_tuskenrifle_ord` | 80 | 5 |
| `tat_weap_inf_tuskensniperrifle_ord` | 200 | 2 |
| `wok_weap_inf_bowcaster_ord` | 75 | 6 |
| `wok_weap_inf_bowcaster_ord_high` | 300 | 2 |
| `wok_weap_inf_thermaldetonator_exp` | 450 | 1 |
| `wok_weap_inf_torpedo_launcher_exp` | 400 | 1 |
| `wok_weap_inf_torpedo_launcher_ord` | 500 | 1 |

### Keys taken from BF2

| BF1 class | Key | BF1 disc | Now |
|---|---|---|---|
| `all_weap_inf_fusioncutter` | LockOnRange | 3 | 4 |
| `all_weap_inf_fusioncutter` | LockOnAngle | 30 | 90 |
| `all_weap_inf_remotedroid_ord` | MaxHealth | 40 | 100 |
| `all_weap_inf_sniperrifle` | ShotDelay | 0.5 | 1 |
| `all_weap_inf_torpedo_launcher` | LockTime | 1.5 | 1 |
| `all_weap_inf_torpedo_launcher_exp` | DamageRadiusInner | 0.75 | 2 |
| `all_weap_inf_torpedo_launcher_exp` | DamageRadiusOuter | 4 | 4 |
| `cis_weap_inf_fusioncutter` | LockOnRange | 3 | 4 |
| `cis_weap_inf_fusioncutter` | LockOnAngle | 30 | 90 |
| `cis_weap_inf_remotedroid_ord` | MaxHealth | 40 | 100 |
| `cis_weap_inf_sniperrifle` | ShotDelay | 0.5 | 1 |
| `cis_weap_inf_torpedo_launcher` | LockTime | 1.5 | 1 |
| `cis_weap_inf_torpedo_launcher_exp` | DamageRadiusInner | 0.75 | 2 |
| `cis_weap_inf_torpedo_launcher_exp` | DamageRadiusOuter | 4 | 4 |
| `com_item_healthrecharge` | SoldierHealth | 15 | 25 |
| `geo_weap_inf_blunderbuss` | ShotDelay | 2 | 1 |
| `imp_weap_inf_fusioncutter` | LockOnRange | 3 | 4 |
| `imp_weap_inf_fusioncutter` | LockOnAngle | 30 | 90 |
| `imp_weap_inf_remotedroid_ord` | MaxHealth | 40 | 100 |
| `imp_weap_inf_rifle` | ShotDelay | 0.2 | 0.166 |
| `imp_weap_inf_sniperrifle` | ShotDelay | 0.5 | 1 |
| `imp_weap_inf_torpedo_launcher` | LockTime | 1.5 | 1 |
| `imp_weap_inf_torpedo_launcher_exp` | DamageRadiusInner | 0.75 | 2 |
| `imp_weap_inf_torpedo_launcher_exp` | DamageRadiusOuter | 4 | 4 |
| `rep_weap_inf_commando_pistol` | ShotDelay | 0.05 | 0.1 |
| `rep_weap_inf_fusioncutter` | LockOnRange | 3 | 4 |
| `rep_weap_inf_fusioncutter` | LockOnAngle | 30 | 90 |
| `rep_weap_inf_remotedroid_ord` | MaxHealth | 40 | 100 |
| `rep_weap_inf_sniperrifle` | ShotDelay | 0.5 | 1 |
| `rep_weap_inf_torpedo_launcher` | LockTime | 1.5 | 1 |
| `rep_weap_inf_torpedo_launcher_exp` | DamageRadiusInner | 0.75 | 2 |
| `rep_weap_inf_torpedo_launcher_exp` | DamageRadiusOuter | 4 | 4 |
| `tat_weap_inf_tuskenrifle` | ShotDelay | 0.2 | 0.33 |
| `tat_weap_inf_tuskensniperrifle` | ShotDelay | 0.5 | 1 |
| `wok_weap_inf_torpedo_launcher_exp` | DamageRadiusInner | - | 2 |
| `wok_weap_inf_torpedo_launcher_exp` | DamageRadiusOuter | 5 | 4 |

### Pickups

| BF1 item | Heals, disc | Now | BF2 |
|---|---|---|---|
| `com_item_powerup_dual` | 100 | 133.33 | 200 |
| `com_item_powerup_health100` | 75 | 100 | 200 |
| `com_item_powerup_health25` | 50 | 66.67 | 100 |
