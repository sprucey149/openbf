---
id: NODMG
title: Shots that hit but do no damage (the maintainer's playtest 10-21): every bolt / bullet that ends in a unit-impact effect against the target's health on its next tick - per weapon, target class, map and game; name the case, fix it at the root, keep the census as a test (the Hailfire droid's chaingun on the player included)
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
The maintainer (INBOX 2026-10-21 18:40 item 1): a bullet hits a unit, the impact particle shows, and the health does not always drop.
It hurts every fight. INBOX 20:00 item 5b adds a weapon that seems to do no damage to the player at all: start with the
Hailfire droid's chaingun.

## Do
1. Reproduce headless first: a census over a round per map (both games) - every shot that ends in a unit-impact effect
   (the effect the player sees) against a health change on that unit on the next tick. Split by weapon, target class,
   map, BF1 / BF2, and by the target's state: shield up, armour / damage scale, friendly fire, boarding, ragdoll / thrown,
   seated in a vehicle or turret, the tick it dies or spawns, a blast after a direct hit (order of damage and blast).
   The black box's `--events` view holds the hits; a scratch print beside it is fine.
2. Name each case where the effect fires and no damage lands, with its count. Some may be the PS2's own rule (a
   vehicle-scaled weapon on a soldier, a zero-damage class): check those against the ODF / `analysis/` before calling
   them bugs, and say which.
3. Fix the real ones at the root (the hit test and the effect must agree on what was hit).
4. The Hailfire chaingun at the player: a scene test (the player in front of a bot Hailfire, invincibility off) with
   the damage per second it should do from its ODF.

## Acceptance
- The census line before and after: impact effects with no health change on a damageable target, per game, from N to 0
  (or to the named PS2-rule cases only, each with its ODF line). The census kept as a test on a fixed scene.
- The chaingun test passes and fails under a mutation that drops its damage.
- Sweep / fidelity: only what the fix declares.

## Measured (2026-10-21 21:20, measure session; main 8438049; evidence `engine/artifacts/lead_tmp/nodmg/`)
Instrument: an `NODMG_LOG` eprintln patch in `Sim::resolve_bolt` (scratch worktree `openbf-wt/nodmg-scratch`, detached at
8438049; the patch is `nodmg/combat_patch.diff`): one line per bolt landing - weapon, victim health before / after, the
desc damage, the scales, friendly / invincible, stance, whether the victim or the shooter is mounted, and for a mounted
victim the vehicle kind / class / seat and the hit point in the vehicle's frame. 32 maps (all but core_bf1/2), seed 42,
9000 ticks (`census2/`, 8 parallel; deterministic: census1 and census2 agree, 934 = 934). 4 more seeds of bf2_geo1 at
18000 ticks for the Hailfire (`census3/`). Bot-vs-bot only: the sweep has no human shooter (slot 0 vacated).

| | BF1 (14 maps + tat1/tat2) | BF2 (16 maps) |
|---|---|---|
| bolt hits on a soldier | 12023 | 15719 |
| ... dealing 0 damage | 0 | **14** (all `rep_weap_walk_atte_rear_gun`, desc 0) |
| ... on a RIDER (mounted unit) | **567** | **367** |
| bolt hits on a vehicle hull with no health change | 0 | 0 |

Friendly hits: 194 of 27742, all full damage (`friendly_fire_scale` 1.0, the PS2's). Impacts on terrain/world within
0.25 m of a unit's capsule: 396 of 74505 (0.5 %) - a bolt into the cover a unit hugs; not a bug.

**Case A - repeated ODF keys read FIRST, the PS2 reads them LAST (BF2 vehicle chainguns do 0 damage).**
`com_weap_veh_chaingun_ord` (bf2_cis / bf2_rep / bf2_tur) lists `MaxDamage "0.0"` and later `MaxDamage "50"` (no section
key between). `ClassDb::prop_hash` (`game/src/classdb.rs:175`, `props.iter().find`) returns the first, so the port's
desc is 0. The PS2's parsers store each PROP line straight into the field - B1 FUN_002738a8 (B1:195827 -> B1:195823,
*(decompiler detail omitted)*), B2 FUN_00310620 (B2:302439-302440, *(decompiler detail omitted)*; `re_check.py fn` run on all four lines) - so the LAST line wins [V setter; D: the PROP loop calls it in
file order, the loop itself not re-read this session]. Zero-damage weapons on main: **BF2 Hailfire chaingun**
(`cis_weap_tread_hailfire_chaingun_ord`, no own MaxDamage), **BF2 AT-TE rear gun** (`rep_weap_walk_atte_rear_gun_ord`),
**BF2 building chainguns** (`tur_weap_chaingun_ord`: `tur_bldg_chaingun`, `_tripod`, `tur_bldg_built_chaingun`). The
snail tank's cannon sets its own MaxDamage (fine). Should be 50 x PersonScale 1 = 50 per bolt on a soldier, 5 on a hull
(VehicleScale 0.1); ShotDelay 0.05 -> 1000 / s on a soldier before heat and spread. census3: 52 Hailfire chaingun
landings in 4 rounds, desc 0 on every one; 43 of them hit terrain 2.6-31 m from anybody (the gun may also aim low -
re-measure once it does damage, [H]).
Other scalar keys repeated with different values (full list `nodmg/dupkeys.txt`, 3139 rows incl. legit section lists):
`rep_weap_fly_gunship_cannon_exp` / `_weak_exp` VehicleScale 0.0 | 0.25; `imp_weap_droid_probe_blaster` (both games)
ShotDelay / spreads / ranges / SalvoCount; `com_weap_inf_timebomb` ShotDelay 0.0 | 0.5; `tur_weap_hoth_dishturret`
ShotDelay, its ord's Damage x4; `gun_weap_inf_haywiredetonator_ord` Velocity 1.5 | 25; mortar launchers' OrdnanceName /
ExtremeRange; the charge weapons' (bowcaster, arccaster) per-charge keys are WPAT's (check for a charge-section key
before applying last-wins to them).

**Case B - every rider keeps a hittable soldier capsule at its vehicle's ORIGIN.** `Sim::carry_riders`
(`core/src/sim/seats.rs:691`) sets a rider's `soldier.pos` to the vehicle's `pos`, and every bolt-target list
(`step.rs:756` on foot, `guns.rs:409` seats, `gunner_foot.rs:534` the AI's line probe, `aim_hit` `step.rs:946`) takes
`alive_units()` with no mount filter. Hits land 0.5-1.5 m above the origin, within 0.6 m of its axis: between a
walker's legs (AT-ST / AT-TE / AT-AT origin at the feet), at a building turret's base, inside a hover. The bolt shows a
SOLDIER impact, the vehicle's health (what the HUD readout shows: the acquisition already skips riders, "its vehicle
stands for it", `sim/input.rs` `acquire_magnet`) does not move, and the hidden rider loses health: **290 of the 934 rider
hits killed the rider** (a pilot shot dead through his AT-ST in 5 min of play). By kind: Turret 381, Walker 318, Hover 235.
PS2: riders INSIDE a hull are on no collision list (`re-vehicle-death.md` 10.3, 153 BF1 states [V-RAM]); exposed riders
(bike, open turret) are on list 0x10 at their seat - whether a bolt's ray tests that list is [H] (the masks in
re-collision-layers 5 do not decode it). Building-turret gunners: card TCOL owns that rule.

**PS2-rule cases (not bugs):** a bot's bolt into a human of its own team does nothing (`combat.rs:40`, B1 FUN_001bc190 /
B2 FUN_001dfb38, re-blaster-damage 6.3); a zero-damage ODF (the chaingun's `_tracer` beam: MaxDamage 0, every scale 0).
**Not measured:** the human's own shots (the sweep has no human shooter). The census found nothing a human-only path
would add except the rider case, which is the one his reticle meets: aim at an AT-ST, the readout is the walker.

## Build plan (phase=build)
1. Case A: scalar props read LAST-wins for the weapon / ordnance / explosion classes (`classdb.rs` `prop_hash`: the
   child-most class that defines the key, its LAST value). Audit every `prop()` caller on a key that repeats with
   different values (`dupkeys.txt`): section keys (CAMERASECTION / WEAPONSECTION / FLYERSECTION / CHUNKSECTION lists,
   WeaponName, FirePointName, EyePointOffset) must keep their own readers; if a global switch moves a list reader, add
   `prop_last` and use it from `WeaponClass::from_props` / `OrdnanceClass::from_props` / `ExplosionClass` only. Test:
   a fixture class with `MaxDamage 0.0 ... MaxDamage 50` parses 50; the Hailfire chaingun from the baked bf2_cis.
2. Case B: riders out of every bolt-target list and `aim_hit` (a hit on the vehicle stands for them); exposed riders
   [H] - default: out too (the acquisition's rule), FEEL/decision note; TCOL settles turret gunners.
3. Tests: (a) the census as a test on a fixed scene - a bot rifleman firing at an AT-ST with a pilot at 20 m: every
   Soldier impact has a health change, 0 rider hits, the walker's health drops; (b) the Hailfire chaingun at a
   non-invincible player 15 m ahead: 50 per landed bolt; both mutation-checked (first-wins back; riders back in).
4. Re-run the census (patch + `census2` command in the report) on the branch: zero-damage 14 -> 0, rider hits 934 -> 0.
Declares: `sweep; arena; sim` (+ fidelity on the walker / turret maps: riders no longer die to small arms, chainguns
start killing - expect kills / deaths to move on bf2_geo1, bf2_kas2, bf2_end1, bf1_rhn1, bf1_nab1, bf1_kam1, tat1).

## Planner note
`files:` left empty for the measure phase (no code is written), so the parked ARVSA branch's combat.rs tests do not hold it back. At `phase=build` fill `files:` - likely engine/crates/core/src/sim/combat.rs, sim/blast.rs, sim/vehicles.rs; ARVSA (parked, waits on the RTX) adds tests to combat.rs and blast.rs: rebase on it once it lands.
