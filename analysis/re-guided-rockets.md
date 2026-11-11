# Ordinary-class guided rockets, BF2

2026-10-28, GUIDED, [O] UNAUDITED. This investigation used the local PS2 class data, executable and existing research. No live PCSX2 comparison was performed. `analysis/INDEX.md` was read first; relevant existing notes are `re-remote-droid.md`, `re-vehicles.md` section 4.2, `re-vehicle-death.md` section 4 and `re-boundary.md` section 4.

## Census and scope [D]

The locally baked BF1 side databases have no remote weapon firing a flyer. BF2's ordinary soldiers carry four award weapons: `all_weap_award_rocket_launcher`, `cis_weap_award_rocket_launcher`, `imp_weap_award_rocket_launcher`, `rep_weap_award_rocket_launcher`. All fire `com_weap_award_rocket_launcher_ord`, an entity whose engine base is `flyer`, through the `remote` weapon base. This is neither a homing missile nor a recon droid.

Carrying records, including four inheritance templates:

| Faction | Soldier records |
| --- | --- |
| Rebels | all_inf_default_rocketeer, all_inf_rocketeer, all_inf_rocketeer_fleet, all_inf_rocketeer_jungle, all_inf_rocketeer_snow |
| CIS | cis_inf_default_rocketeer, cis_inf_rocketeer |
| Empire | imp_inf_default_rocketeer, imp_inf_rocketeer, imp_inf_rocketeer_snow |
| Republic | rep_inf_default_rocketeer, rep_inf_ep2_rocketeer, rep_inf_ep2_rocketeer_chaingun, rep_inf_ep3_rocketeer |

The concrete classes are Rebel Vanguards, Assault Droids, Shock Troopers and Clone Heavy Troopers and their listed variants. The local integration test repeats the inherited-slot census, checks every weapon and launches every record. Chewbacca's hero launcher and vehicle/turret guided weapons are outside this implementation. Their runtime loaders remain unsupported, as before, instead of changing their behavior through the new infantry path.

These weapons author `MedalsTypeToUnlock = 2`. The maintainer explicitly chose to keep medal progression separate and use the existing award session cheat. Since #142 every award weapon is in the roster and follows one rule (`Sim::weapon_locked`, re-grenades.md §2.4): cycling, firing and the bots' weapon choice skip it until its unit holds the medal. Only the pause cheat or CLI `--cheat awards` grants medals, and only to the player outside the space modes, so bots no longer launch award rockets with the cheat on. The standard launcher authors no `MedalsTypeToLock`, so the guided one is an extra slot, not a replacement. No medal-earning or profile progression is implied (#130).

## Disc properties [D]

Shared ordinary award rocket: MaxHealth 1, HealthType droid, EnergyBar 200, EnergyAutoRestore -5, EnergyBoostDrain 20, Acceleraton 80, Min/Mid/MaxSpeed all 30, BoostSpeed 80, BoostAcceleration 50, PitchRate/TurnRate 1.2, PitchFilter/TurnFilter 4, their deceleration-filter hashes 53d5cdc8/1795170d both 4, BankAngle 0.1, BankFilter 3.5. The weapon has one round per clip; authored class totals are Rebel 5, other sides 4. InitialSalvoDelay 0.3, ShotDelay 1.4, ReloadTime 0.0. The spawn offset hash 389c8e46 is (-0.5, 1, 5). Geometry is `com_weap_missile`; the authored bank is bazooka.

The ordinary missile uses TrackCenter (0,1,-3), TrackOffset (0,2,6). EngineSound is `rep_inf_grocket_engine_parameterized`, DeathSound `imp_weap_ord_exp_lg`. Exhaust effect hash 0894d756 names `com_sfx_exhaust_jedistarfighter`; it already exists in the common BF2 effect bake. The missing model was baked directly from the local INGAME.LVL: 95 vertices, 52 triangles, texture `missile`. `rebake.sh` now includes this model in its persistent side bake list.

Two explosion bindings are distinct: e04f08c1 = `com_weap_veh_guided_rocket_exp` (500 damage, DamageRadius 22.5, vehicle/building scale 0.2), 8a323f32 = `com_weap_veh_guided_rocket_noncrit_exp` (50 damage, inner/outer 3/5, vehicle scale 0.5). Effects, pushes, per-target scales and sounds are read through the existing explosion loader and damage path.

## Executable evidence [O]

- `FUN_003465f8` derives the remote weapon class from the dispenser. `FUN_00346480` records the spawned controllable and transfers the soldier's controller to it when class+4c0 is set. `FUN_00346380` / `FUN_003463f8` forbid a second spawn while that reference remains valid. `FUN_00346100` handles the fresh control press and a second trigger-word kill path; `FUN_00345fe8` destroys the remote on weapon destruction.
- `FUN_0020fd58` loads the flyer movement properties and explosion bindings. Property c6e9c80b sets the guided flag in class+1208 and remote pilot type 3. `re-boundary.md` independently identifies the three flyer ordnance families using it.
- `FUN_00202d08` handles this guided flag before ordinary flyer movement. With a pilot and positive energy it follows the flying update. Removing the pilot or exhausting energy takes its immediate detonation/removal path; a rocket cannot remain parked and be retaken like the recon droid. Its noncritical/critical energy-expiry choice reads whether boost is engaged and forward speed against BoostSpeed times class+1244 (authored bb3082af = 0.8). The normal death/crash branch uses the critical binding before crash cleanup.
- The flying branch clamps forward speed toward the Min/Mid/Max throttle law, uses BoostAcceleration and BoostSpeed while boosting and triple deceleration while slowing. Its pitch and turn rates slew toward their commands by filter*dt; the actual branch near 00206400 corrects the old note's broad description as a first-order filter. The implementation uses a separate quaternion flight state and forward thrust, not the droid's gravity/lift spring.
- `re-vehicle-death.md` pins health damage through the authored HealthType scale. The new rocket accepts bolts and blasts as a droid-scaled target; its own explosions credit the launching unit's generation and team through the existing blast path.

Useful reproducible reads: `python scripts/re_check.py word b2 0x629ca8 36 --u32`; extract the named `FUN_00346100`, `FUN_00346480`, `FUN_00202d08` and `FUN_0020fd58` bodies from the local corpus; check the corresponding raw instructions with `scripts/tools/eedis.py bf2 <start> <end>`. No decompiler source or disc asset is committed.

## Port controls, implementation and acceptance limits

Select the award launcher with the award cheat ("Award weapons", `--cheat awards`) on, as the player on a ground map of either game (a BF1 heavy carries BF2's launcher beside its own, award-weapons.md §3), then fire. Its windup consumes one round and launches one controlled rocket. Right stick or mouse steers; forward stick selects throttle; sprint boosts. The common remote pad-consumption and control-release helpers are shared with recon droids. The soldier remains stationary under control. ACTION returns control and ends the rocket with the noncritical explosion. A fresh secondary-fire press detonates it. Expiry, impact, destruction, owner death/recycling and mounting also return or clear control; held fire across return cannot accidentally launch another rocket. These ACTION and secondary-fire bindings follow the port's remote-control convention; their precise original pad mapping remains unresolved and is not claimed as PS2 validation.

The camera uses the rocket's own pose and class offsets; health/energy HUD, reticle, launcher animation, model, authored exhaust and parameterized engine/death/explosion audio are connected. Render interpolation restores sim state exactly. Guided steering uses its filtered turn-stick input; the foot/droid mouse-look preview is not applied to its inertial rotation. A maintainer playtest exposed a full-tick prediction overriding the interpolated rotation at display rate; the follow-up regression covers 60/75/144 Hz while preserving the flight law. No lock-on is introduced. Bots choose the unlocked weapon through the existing arsenal and steer at their current visible enemy.

Remaining original-game acceptance: live launch/camera/control/audio comparison; exact launcher-offset reference frame; full flyer lateral inertia, high-pitch/roll/boost steering fidelity; exact collision-primitive dimensions (the port's rocket target sphere is provisionally 0.2 m, with swept center-line world collision); damage/crash transition and control-loss explosion timing. The existing runtime has no player-boundary/fly-height input loaded into Sim, so the original guided 2.17-second outside-boundary rule cannot be applied from current map inputs. It is an explicit remaining map-boundary integration requirement, not a guessed nav-graph boundary. Main's separately owned portable-turret implementation is merged; its soldier acquisition/retention respects cloak visibility and rocket sweeps include deployed turrets. The cards remain open for original-game acceptance, even though the local implementation and available checks can be completed.
