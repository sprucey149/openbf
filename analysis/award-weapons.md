# Award weapons, the damage award and slicing on both games' maps

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
gives 600 x dt / 0.15 on each of its five ticks, 11 % over 600; that is how it already was.)

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

- The damage award has its own cheat row, "Damage award (x1.5)" (id `damage`): while it is on,
  the player holds medal 8 and deals x1.5 (`core::sim::buffs::WAR_HERO`, written each tick
  while medal 8 is held and removed with it). Never a bot, a hero or a space round. The
  "Award weapons" row gives every other medal, so the award weapons can be judged at their
  own damage; both rows on is BF2's player holding every award.
- The other medal effects are not built (see section 5): damage taken x0.5 would change how
  hard every fight is for the player, which was not asked for.
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
  that is x1.37 a volley (BF2: x1.14 over its shotgun) with BF2's reach and shield damage.
  Scaling the flechette to the Blast Cannon would make the same award a different gun on
  BF1 maps.
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
- the damage award's effect (section 1);
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
- the other medal effects: Guardian (bit 7, damage taken x0.5, B2:168138-168144), Endurance
  (bit 6, energy regeneration x1.5: six update sites, for one B2:160231, each multiplying by
  the 1.5 at `DAT_005feda0`), Technician (bit 3, the vehicle the holder sits in repairs
  itself, B2 `FUN_002f5478`, 283804);
- the riders' "Slicer Alert" line;
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
