# Award weapons, the stat medals and slicing on both games' maps

Status: [ours], built 2026-11-10. Builds on `gameplay-normalization.md`.

The maintainer (2026-11-10): "award weapons don't seem to be doing as much damage as they
should. The award weapons in bf2 did more damage. Also I want to extend award weapons to bf1
maps for the equivalent bf1 weapons like the rifle, pistol, sniper, heavy rocket launcher. Is
there a shotgun in the original? And slicing and the other awards should be a thing in bf1 maps
too."

B2:n is line n of `analysis/SWBFRNT2.ELF-decomp.c`.

## 1. Award damage against BF2

### What the disc says

BF2's PS2 class data (`engine/assets_baked/classes/bf2_*.ron`), damage to a person:

| Award (medal) | Award | Base weapon | Advantage | What else the award changes |
|---|---|---|---|---|
| Elite rifle (1, Frenzy) | 150, far 75 | 80, far 50 (Empire 70, far 40) | x1.875 (Empire x2.14) | three-round burst 0.08 s apart, 0.75 s between bursts, 36 rounds |
| Award wrist blaster (1, super battle droid) | 150, far 75 | 100, far 60 | x1.5 | as the Elite rifle |
| Precision pistol (0, Gunslinger) | 150 | 60, far 40 | x2.5 | instant hit, no fall-off, no heat, 16 rounds, 0.1 s, spread 0.1 against 0.4 |
| Beam rifle (4, Marksman) | 600 over 0.15 s | 300 | x2.0 | a beam, 4 rounds, 1.5 s between shots, fixed x6 zoom |
| Flechette shotgun (5, Regulator) | 85 x 7, far 25 | 65 x 8, far 15 | x1.31 a pellet, x1.14 a volley | 1.2 s against 1.0 s, full damage to shields against a quarter |
| Guided rocket (2, Demolition) | blast 500, radius 22.5 m, x0.2 to vehicles and buildings | none replaced | - | an extra slot beside the launcher |

### What the port did before this change

The same numbers. `OrdnanceClass::from_props` reads the award classes with no override, and
BF2's class files are not rewritten by the shared-numbers pass. Measured in the sim with real
classes on flat ground (a still target 10 m off, one trigger pull, body hits):

| Weapon | Hits a pull | Health a hit |
|---|---|---|
| Republic rifle / Elite rifle | 1 / 3 | 80 / 150 |
| Imperial rifle / Elite rifle | 1 / 3 | 70 / 150 |
| Pistol / Precision pistol | 1 / 1 | 60 / 150 |
| Sniper rifle / Beam rifle | 1 / 5 ticks | 300 / 133.33 a tick (666.67 a shot) |
| Shotgun / Flechette shotgun | up to 8 / up to 7 | 65 / 85 |

A head hit is x3 on every one. So no award weapon hit for less than its disc value. (The beam
gives 600 x dt / 0.15 on each of its five ticks, 11 % over 600. The PS2 does the same: BF2's beam
update `FUN_00311d00` scales each update's damage by dt / LifeSpan (B2:303292-303294) and only
then steps the beam's life and removes it at zero or below (`FUN_0030f988`, B2:301820-301822), so
a 0.15 s beam lands ceil(0.15 / dt) updates, five at the PS2's 30 Hz: 666.67. Not a port error;
nothing changed. Clipping the last update to give exactly 600 would move every beam of both games,
the BF1 tank laser 867 -> 800 and BF2's spider beam 3067 -> 3000.)

### What BF2 adds that the port did not

BF2's soldier update gives the holder of the damage medal (medal 8, "War Hero", 36 points in
one life) a permanent x1.5 on everything it deals: `FUN_0024ece0` (B2:168091-168144) reads the
medal bits of a non-hero soldier with a stats record; bit 8 writes the offense effect at 340 s
and x1.5 (B2:168133-168137), bit 7 the defense effect at 340 s and x0.5 (B2:168138-168144), on
every update, and an effect timer over 300 s never counts down. A player holding every award
therefore hit for 225 with the Elite rifle and the Precision pistol and 900 with the Beam
rifle. The port's cheat set all sixteen medal bits but only the weapon switch read them.

That the x1.5 is what the maintainer remembers is an inference: the rule is in the code, the
link to the remark is not proved.

### What is built

- The damage award has its own cheat row, "War Hero: damage increase (x1.5)" (id `damage`):
  while it is on, the player holds medal 8 and deals x1.5 (`core::sim::buffs::WAR_HERO`,
  written each tick while medal 8 is held and removed with it). Never a bot, a hero or a
  space round. The "Award weapons" row gives the five weapon medals only (0, 1, 2, 4, 5), so
  the award weapons can be judged at their own damage. The other three stat medals have a row
  each (section 9); all five rows on is BF2's player holding every award.
- No award number is changed. On BF1 maps the award is BF2's own class, so on the shared scale
  its advantage over the BF1 weapon it replaces is BF2's (next section).

## 2. Is there a shotgun in the originals?

Yes, in both.

- BF1: the Blast Cannon, `all_weap_inf_shotgun` (Rebel pilot) and `imp_weap_inf_shotgun` (dark
  trooper): 5 bolts of 65 (86.67 on the shared scale), 0.75 s between shots, 30 rounds. The
  Republic and the CIS have none in BF1.
- BF2: `*_weap_inf_shotgun` on all four engineers (8 instant pellets of 65, far 15, 1.0 s) and
  its award `*_weap_award_shotgun` (`MedalsTypeToUnlock 5`, the Flechette shotgun: 7 of 85).

## 3. Award weapons on BF1 maps

BF1 has no medals and no award weapons (`re-hitzones.md` Part B section 4). Now a BF1 class of
the four trooper sides carries BF2's award for each weapon BF2 has one for
(`game::roster::awards::BF1_AWARDS`):

| BF1 weapon | BF2 award | Medal | Takes the weapon's place |
|---|---|---|---|
| `*_weap_inf_rifle` | `*_weap_award_rifle` | 1 | yes |
| `*_weap_inf_pistol`, `rep_weap_inf_commando_pistol` | `*_weap_award_pistol` | 0 | yes |
| `*_weap_inf_sniperrifle` | `*_weap_award_sniper_rifle` | 4 | yes |
| `*_weap_inf_shotgun` | `*_weap_award_shotgun` | 5 | yes |
| `*_weap_inf_torpedo_launcher` | `*_weap_award_rocket_launcher` | 2 | no: an extra slot, as in BF2 |

Who gets what: soldiers the rifle and pistol; heavies the guided rocket and pistol; snipers
the beam rifle and pistol; the Rebel pilot and the dark trooper the flechette shotgun and
pistol; the other pilots (mortar launcher or arc caster) and the jet trooper the pistol;
smugglers none; the super battle droid keeps the BF2 kit it already takes whole. 63 award
slots on 34 BF1 classes (`tests/data/class_kits.txt`).

Rules:

- **The weapon is BF2's class**, built from BF2's class data (the BF2 side files a BF1 round
  now loads beside its own, `roster::load_classdb`). Damage, burst, clip, spread, zoom, the
  beam and the guided rocket are BF2's, the same on every map. BF2's files are not rewritten,
  and BF1's base weapons are already on BF2's scale, so the advantage is BF2's: rifle 150 over
  80 (Empire 70), pistol 150 over 60, sniper 600 over 300.
- **The shotgun is the one named difference.** BF1's Blast Cannon is its own gun (5 x 86.67
  every 0.75 s); the flechette is BF2's 7 x 85 every 1.2 s on both. Against the Blast Cannon
  that is x1.37 a volley (595 against 433; BF2: x1.14 over its shotgun) but, counting the
  rate, about x0.86 a second point-blank (496 against 578 a second): on a BF1 map the award
  hits harder a shot and less a second, and it fires BF2's thin instant pellets in place of
  the Blast Cannon's bolts. A BF1 award Blast Cannon of its own (same bolts and sound, the
  flechette's advantage over its base gun) waits for the maintainer's pick.
- **Clips** are BF2's: the count on the first of the side's rifleman, marine, rocketeer,
  sniper, engineer or pilot that carries the award.
- **Slots** follow the class's own, in the order of the weapons they stand for.
- **Looks and sounds** are the BF1 weapon's: its hand model, first-person model, HUD row,
  animation bank, sounds, muzzle flash and impact effects (a BF1 map has those loaded). The
  award keeps its purple bolt. The guided launcher keeps BF2's HUD row so it can be told from
  the launcher; its rocket trails and bursts as the BF1 rocket does.
- **The switch rule** is BF2's on both games (`core::sim::awards`, `re-grenades.md` section
  2.4): an award is out of the cycle until its medal is held, and then the weapon it replaces
  is. A BF1 weapon carries the lock by name (`roster::awards::bf1_medal_lock`).
- Bots hold no medal on either game, so their kits and their weapon choice are unchanged.

Not done for the guided rocket on BF1 maps: its engine sound (a BF2 engine class) and BF2's
large explosion effect. It bursts with the BF1 rocket's effect over a 22.5 m blast.

## 4. Slicing

Slicing is the fusion cutter on an enemy vehicle with soldiers aboard (issue #35,
`re-build-zones.md` section 11, `core::sim::repair`). It already ran on BF1 maps; two things
differed and are now the same on every map:

- **Slice time.** BF1 authors no `TimeRequiredToEject`, so BF1 vehicles took the engine
  default for their base (hover 3 s, walker 12 s, flyer 4 s). Each now takes its BF2
  counterpart's (`game::vehicle_class::BF1_SLICE_TIMES`):

| BF1 class | Was | Now (BF2's) |
|---|---|---|
| AAT | 3 | 10 |
| STAP, Imperial and Republic speeder bikes | 3 | 1 |
| Hailfire droid | 3 | 5 |
| Spider walker | 12 | 12.5 |
| Imperial and Republic fighter tanks, Rebel combat speeder | 3 | 7.5 |
| AT-ST (three skins) | 12 | 10 |
| X-wing, Y-wing, snowspeeder, TIE fighter, TIE bomber, droid starfighter | 4 | 2.5 |
| Geonosian fighter (no BF2 counterpart; PROVISIONAL, replace: the maintainer's pick) | 4 | 2.5 |

  The tauntaun, kaadu, gunship, MAF and Jedi starfighter keep their base default, which is
  what BF2's class of that name has. Command vehicles are never sliced on either.
- **Cutter reach.** BF1's cutter reached 3 m in a 30 degree cone, BF2's 4 m in 90. The four
  sides' BF1 cutters now take BF2's (`normalize::KEYS`), for repairing as well as slicing; the
  Gungan cutter, which BF2 lacks, keeps its own 6 m. Repair amounts are still each game's own
  (BF1 60 a shot on a vehicle, BF2 200), with each game's vehicle health.

Only a player slices; bots never do, on either game.

## 5. The other awards: what exists and what issue #130 still needs

Exists, the same on both games:

- the medal bits and the switch rule (`Sim::award_bits`, `Sim::weapon_locked`);
- award slots on every trooper class of both games;
- the four stat medals' effects as player-only cheat rows (section 9);
- critical-hit counting for the demolition award (`WeaponClass::rocket_launcher`,
  `Event::CriticalHit`);
- the round-end stats and medal pages (`game::round_stats`; medal counts are always 0);
- class unlock points (`game::unlocks`).

Not built (issue #130):

- earning: the per-life counters and thresholds (`re-hitzones.md` Part B section 3: pistol 6
  kills, rifle 12, launcher 4 vehicle critical hits, 1 slice, sniper 6 head hits, shotgun 8
  kills, and 12 / 24 / 36 points), the three-quarter threshold at a higher career level, the
  bit update with its on-screen message, loss on death and permanence at the top level;
- career counts in the profile and on the stats pages, and the settings toggle;
- the yellow "Award: <bonus>" line when a bonus becomes available (`game.awards.bonus.*`);
- the riders' "Slicer Alert" line;
- the weak-part lock's count (the maintainer's ask, 2026-11-10: "I want the lock onto vehicle weak
  parts that give the award"): since the third weak-spot pass any round that strikes a weak part
  deals its vehicle class's factor (x4 on the AT-TE, the AT-AT and the spider walker, x2 on the
  rest) and raises `Event::CriticalHit`, locked or not (`re-ordnance-kinds.md` §5.1), so a
  part-locked rocket on the part is a real critical hit and counts as one, from the rocket
  launcher's `Event::CriticalHit`. One that meets the plate beside the part (about 1 in 25 in the
  survey) is plain and does not count; replace: count every part-locked hit on its vehicle if the
  maintainer wants that;
- whether bots earn medals;
- medal scoring keys on BF1 weapons (`ScoreForMedalsType`), which earning will need.

## 6. Bots and locked awards

Bots hold no medal, so every award slot is locked for them: the switch, the weapon choice,
the sniper's scope and the guided launch already skipped it. One reader did not: the reach a
walking bot fights within (`ai::behave::brain::fight_reach`, playtest C3) took the longest AI
range of every slot, locked or not. On BF2 maps a rifleman, a pilot and a dark trooper
therefore turned to fight out to the Precision pistol's 80 m (+10) instead of their rifle's
64 m or their own 60 m. With award slots on BF1 classes the same fault would have stretched
BF1's troopers from 48 m to 80 m. A locked award no longer counts, on any map. The reach
applies to a ground round's walking bot only (`walking_on` in `brain::think` excludes a
space round), so space rounds aim as before.

## 7. Hashes and bot rounds

Measured 2026-11-10: a release binary of the branch this builds on against this one,
`--sim-trace <out> 15000 --map <dir> --mission <ron> --state-hashes`, the trace and every
tick's hash compared, all 62 missions.

| Missions | Result |
|---|---|
| All 15 BF2-source Clone Wars conquest missions (`bf2_*_con_cw`, `tat2_con_cw`) | byte-identical |
| 10 of the 18 BF2-source Galactic Civil War ones (`bf2_cor1`, `dag1`, `fel1`, `kam1`, `mus1`, `myg1`, `pol1`, `tan1`, `tat3`, and `tat2_con`) | byte-identical |
| The other 8 (`bf2_dea1`, `end1`, `geo1`, `hot1`, `kas2`, `nab2`, `uta1`, `yav1`) | move, first at tick 327 to 10594: the bots' reach (section 6) |
| `tat2_eli` (Hero Assault) | byte-identical |
| All 28 BF1-source conquest missions | the hash moves from tick 1 (each trooper holds more slots, whose state is hashed); the units, score and posts columns first move at tick 840 to 8014 |

What moves the BF1 fights is the cutter's reach alone: with the four cutter rows taken out,
all 28 BF1 traces (units, rng, score, posts, timers) are byte-identical to the base for 15000
ticks, so the award slots change no bot's behaviour. With the rows in, pilot bots repair from
4 m.

`scripts/sim_gates.sh`, before and after: the built-in fixture (`5c5458c8927f122b`) and the
aim replay (`443de283b68845b0`) do not move. The TAT1 walk and aim demos move in the hash
only (`474c5cce53472502` to `c906917527627f71`, `ac5bf2e68e45f19d` to `8a742201c8d1a7ec`);
their `motion.csv` files are byte-identical. The references in `gates.toml` are the
maintainer's to refresh.

Space rounds: `award_bits` is 0 there, the reach applies to ground rounds only, and the
space files are as they were. With the cheat on, the player's offense effect is in the state hash on any ground
map.

## 8. Tests

- `game/tests/suite/award_weapons.rs`: the advantage per medal on every class of both games,
  BF1 award slots and their looks, the cheat on a BF1 kit in the sim (150 a round x 1.5, the
  kit back with the cheat off), the same pull on BF1's and BF2's trooper, the beam and the
  flechette on BF1 kits.
- `core/src/sim/awards/tests.rs` `all_awards_give_the_damage_award`.
- `core/src/ai/behave/brain/tests.rs` `a_locked_award_adds_nothing_to_a_bots_reach`.
- `game/tests/suite/squad_vehicle_maps.rs` `a_player_slices_a_real_enemy_aat`: 10 s from
  3.5 m on BF1 Naboo and BF2 Naboo alike.
- `game/tests/suite/vehicle_class.rs` `bf1_slice_times_match_the_bf2_twins`.
- `game/tests/suite/normalization.rs`: the cutter rows against BF2's class.
- `game/tests/suite/roster.rs` `every_class_kit_is_the_discs_or_explained` and
  `tests/data/class_kits.txt`: 63 added rows, each marked as BF2's award.
- `core/src/sim/awards/tests.rs` `each_award_row_sets_only_its_medal`,
  `the_damage_reduction_cheat_halves_damage_taken`, `the_energy_regen_award_refills_energy_1_5x`,
  `a_bot_and_the_proxy_never_hold_a_medal`.
- `core/src/sim/vehicles/tests.rs` `the_energy_regen_award_refills_the_drivers_vehicle_1_5x`,
  `the_vehicle_regen_award_repairs_the_drivers_vehicle`; `core/src/sim/flyers.rs`
  `the_vehicle_regen_award_and_the_pilot_rule_do_not_stack`.
- `game/src/fx/tests.rs` `a_bf1_library_carries_the_buff_auras_and_their_loops`,
  `an_ability_cast_queues_its_effect_sound`; `game/src/sounds/tests.rs`
  `a_bf1_core_bank_imports_the_buff_sounds`; `game/tests/suite/vehicle_class.rs`
  `bf1_repair_scales_match_the_bf2_twins`.

## 9. The stat medals: War Hero, Guardian, Endurance, Technician

Built 2026-11-10 ([ours] as cheat rows; the effects are BF2's). BF2 has nine medals: five
unlock a weapon, three are earned by points in one life (Endurance 12, Guardian 24, War Hero
36) and one by a slice (Technician); `re-hitzones.md` Part B section 3. BF1 has none. Each
stat medal is a pause-menu cheat row beside "Award weapons", off by default, named after the
medal and BF2's own words for its bonus (`game.awards.bonus.*`):

| Row (id) | Medal | Effect | Look and sound | Evidence |
|---|---|---|---|---|
| War Hero: damage increase (x1.5) (`damage`) | 8 | everything dealt x1.5 | red lightning aura (`com_sfx_buffed_offense`), loop `buff_rage_effect` (gain 0.5, 15-30 m) | B2 `FUN_0024ece0` 168133-168137 |
| Guardian: damage reduction (x0.5) (`guardian`) | 7 | all damage taken x0.5 | teal aura (`com_sfx_buffed_defense`), loop `buff_rally_effect` | B2 168138-168144 |
| Endurance: energy regen (x1.5) (`endurance`) | 6 | every energy refill x1.5, for him and the hover, walker or flyer he drives; a drain is unchanged | none | f32 1.5 at B2 `DAT_005feda0`; B2:160231 (soldier), 101661, 109523, 116988, 133701, 178209 |
| Technician: vehicle regen (`technician`) | 3 | the hover, walker or flyer he drives repairs its class's `PilotSkillRepairScale` of its maximum a second (0.005 when it authors none) | none | B2 `FUN_002f5478` 283804, 283897; `FUN_00269410` |

Rules, the same on both games' maps:

- **Who.** The player only (`Sim::award_bits`): never a bot, the difficulty proxy (it is a bot
  for every cheat), a hero, or a space round. Difficulty tiers are tuned with every award off
  (the maintainer, 2026-11-10); no tier reads a medal.
- **War Hero and Guardian** are the PS2's 340 s effect timers, rewritten every update while
  the medal is held and cleared with it; a timer over 300 s never runs down, so a shorter
  Rally, Rage or power-up does not replace them. A hero gets neither (the PS2 clears them).
  A hero is the hero manager's class for the team or, in Hero Assault, every class
  (`Sim::hero_unit`), as the PS2's class-level hero flag covers them all; before, the damage
  award's `consts.hero` check never saw a hero in play.
- **Endurance** multiplies the soldier's `EnergyRestore` and `EnergyRestoreIdle` (and a
  weapon's positive energy override) and the driven vehicle's `EnergyAutoRestore`. `[ours]`
  Never for a hero: the original gives a hero the bonus (no hero check at B2:160231).
  Not applied: the droideka, whose shared bar refills at its shield's `AddShieldOff` in the
  port (the original multiplies its separate soldier energy), and the remote droid, which runs
  the port's fixed 90 s clock.
- **Technician** counts once: the PS2 adds the repair when the driver is a BF2 pilot in a
  flyer over 1400 health OR holds the medal, never twice. Only the vehicle's own seat counts
  (a passenger holder adds nothing); not while a flyer falls; never past full; not turrets,
  remote droids or guided rockets. `[ours]` The PS2 counts the rider when he boards; the port
  asks every tick, so a row toggled in a seat applies at once. `[ours]` A BF1 flyer repairs at
  its BF2 twin's scale (0.02 for every fighter and the snowspeeder, 0.01 for the gunship;
  BF1 authors 0.03-0.04, which no rule of the port reads on a BF1 map; `game::vehicle_class::BF1_REPAIR_SCALES`).
  Ground vehicles author none in either game and take 0.005.
- **Looks and sounds on BF1 maps** (`[ours]`; BF1 has no auras): the four auras come from
  BF2's common effect layer, loaded after every BF1 layer with nothing else of it
  (`game::fx::Library::load`); their textures and models are searched after every BF1
  directory, so a BF1 file of the same name always wins (on today's bakes no BF1 effect's
  texture or model resolves to a BF2 file, checked against every BF1 map's layers), and the aura loops and cast sounds from BF2's Naboo bank into
  the BF1 resident bank (`game::sounds`, `BUFF_SOUNDS`). No BF2 bank bakes
  `buff_rally_fire`'s sample, so the Rally cast is silent on both games' maps.
- **New on every map:** an aura's looping sound plays while it shows
  (`Fx::buff_loops`, `SoundLib::play_buff_loops`), and an ability's cast plays its effect's
  own sound (`buff_rage_fire` and the like) beside the weapon's `com_buff_fire`. Both were
  missing for the officer and hero abilities too.

Not done: the "Award: <bonus>" line, earning and career levels (issue #130), the spawn page
listing award weapons. trace: whether the PS2 hides the aura while its soldier sits in a
vehicle (`FUN_002368a0`'s attach test); the port draws it at the rider as before.

Hashes, measured 2026-11-10: a release binary of the base (`2fc4df71`) against this one,
`--sim-trace <out> 15000 --map <dir> --mission <ron> --state-hashes` (`--mode eli` for Hero
Assault), every cheat off: `bf2_nab2_con_cw`, `bf2_mus1_con`, `bf1_hot1_con` and `tat2_eli`
traces and every tick's hash byte-identical (tick 15000: `4c202c8120eb2028`,
`b516a33c5bd18455`, `3d0a7fe55bab684a`, `4cf70baa801e3f06`). `scripts/sim_gates.sh` prints
the same values on both (`5c5458c8927f122b`, `443de283b68845b0`, `c906917527627f71`,
`8a742201c8d1a7ec`). With a row on, the player's defense effect is in the state hash, as the
offense one is; vehicle health and energy already are. The sim-trace tool takes no cheats, so
no round was replayed with a row on.
