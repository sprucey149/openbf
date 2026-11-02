---
id: JETP
title: Jet and jump packs, the physics - BF2's jet rules on every map (the jump-jet, the hover, fuel, the Jedi double jump, the fly-height ceiling) for human players; bots, effects, sounds, HUD and animation are JETP2
milestone: M3
value: 3
phase: build
status: done
deps:
files: engine/crates/core/src/soldier.rs, engine/crates/core/src/sim/step.rs, engine/crates/core/src/sim/state.rs, engine/crates/core/src/unit.rs, engine/crates/game/src/roster.rs, engine/crates/game/src/mission.rs, engine/crates/game/src/layout.rs, scripts/gen_missions.py
declares:
needs:
effort:
branch: jetpack
resume: built 2026-10-28 on branch jetpack (build plan steps 1-8 + the cheat; analysis/re-jetpack.md §13); the rest is card JETP2
created: 2026-09-29
defer:
tries: 1
---

## Why
UAUD1 (analysis/unit-audit-bf1.md §3.6). The dark trooper fields on every BF1 GCW map and the jet trooper on every BF1 CW map; the BF1
setter reads JetJump at +0x18d8 (B1 123562) and JetPush (123503). The port turns `JetJump > 0` into a planning-graph
flag only (game/src/roster.rs 396, core/src/nav.rs 53-61). PLAN §LOCKED FEATURE SET: "Jet/jump-pack: BF2 feel;
infinite fuel via cheat".

## Evidence
| class | JetJump | JetPush | fuel cost / initial / recharge | other |
|---|---|---|---|---|
| imp_inf_dark_trooper (+snow) | 21.0 | 0.0 | 0.0 / 0.4-0.5 / 0.025 | JetEffect darktrooper_jetpack |
| rep_inf_jet_trooper | 5.0 | 8.0 | 0.2 / 0.25 / 0.1 | JetType hover, EngineSound rep_inf_Jetpack_engine_parameterized |
| geo_inf_geonosian (local) | 8.0 | 8.0 | 0.1 / 0.2 / 0.03 | JetDuration 5.0, JetType hover |

## Notes
- UAUD2 (2026-09-29, analysis/unit-audit-bf2.md §3.6): BF2 values. Jet troopers (`rep_inf_ep3_jettrooper`, `_ep2_`): JetJump 5, JetPush
  8, JetAcceleration 30, JetType hover, fuel cost 0.16 / initial 0.25 / recharge 0.12 / min border 0.24, effect
  `rep_sfx_jetpack`, their own ControlSpeed jet row (1.5, 1.25, 1.25). Dark trooper: JetJump 21, JetPush 0,
  JetAcceleration 10, fuel 0 / 0.5 / 0.075 / 0.49, `imp_sfx_jetpack_darktrooper`. The jedi heroes: JetJump 10,
  JetEnergyDrain 40.

## SAIR 2026-10-02 - when a PS2 bot jets (`analysis/re-soldier-ai.md` 4.7, AGENT_F Q2, AGENT_H Q6)
- In a fight (attack sub-helper): any jet class with fuel hops every randint(6, 12) s to a random clear spot 20-57 m
  away with open sky above it (the straight line is tested for barriers only where the script calls
  `SetAllowBlindJetJumps(0)` - the Kamino scripts); jump-jet classes go to a JetJump hint (TYPE "4") within 40 m every
  randint(3, 8) s and jet 80 m along its direction; hover-jet classes fly at a target 20 m or more away when the
  fuel lasts and no barrier crosses the line. On a route: the follower sets jump or jet from the connection's flags.
  The deathmatch agent jets straight up at random. BF1: an evade jump every 3-8 s and JetJumpHintHelper.

## RE done (2026-10-28, branch jetp-re): `analysis/re-jetpack.md`
Both games read from code + ELF words (no PCSX2 run, no verifier yet - plan step 9). Jet = a second tap of jump in the
air (stance jump/fall); a tap ends it (BF1 jump-jet cannot be cancelled); fuel empty -> fall. Kick (both games):
v += normalize(R x -0.1 strafe + F x max(fwd,-0.1) + (0,1.1,0)) x JetJump. BF2 jump-jet (dark trooper): JetFuelCost/s
drain, vy += JetPush dt, the air mover with stick x JetAcceleration; BF2 hover: own mover, no gravity, v x exp(-2dt),
flies along the aim (pitch doubled + 0.08 lift) toward ControlSpeed jet row x speeds at JetAcceleration. BF1 hover
holds height (dvy = -2.5 vy), climbs by sin(pitch) x JetPush x 4, horizontal 2 x stick x speeds; BF1 recharges fuel
DURING the jet. Spawn fuel 0.5 (recharge > 0) else 0. BF2 Jedi: the not-hover jet with a saber is a double jump
(vy 1.5 x jump speed, energy 40). JetDuration is read by nothing. HUD: BF2 `player1jetpackbar` events, BF1 a
code-drawn `hud_jetpack_icon`. Decision owed (the doc's 12): BF2 rules on BF1 classes with a stated data mapping
(recommended). Build steps: the doc's 12 (13 small steps).

## Built (2026-10-28, branch `jetpack`): build plan steps 1-8 and the infinite-fuel cheat
The maintainer's decision (2026-10-28): BF2's jet rules on EVERY map, BF1 included (a named deviation `[ours]`); each
class keeps its disc numbers; a BF1 class borrows only the keys BF1 never authors from its BF2 twin (jet trooper
JetAcceleration 30 + jet row 1.5 / 1.25 / 1.25; dark troopers JetAcceleration 10); the Geonosian is skipped (not
baked); no recharge while jetting. What was built, the controls, the feel numbers against the RE's, the judgment
calls and the bot rounds: `analysis/re-jetpack.md` §13.
- Controls: jump on the ground jumps; a second jump press in the air starts the jet (leap / hover / a Jedi's double
  jump); a third ends it into a fall; landing or an empty tank ends it too (landing: the 0.3 s lock).
- Tests: `core/src/soldier/tests/jet.rs` (17), `core/src/sim/step/tests/jet.rs` (5),
  `game/tests/suite/roster.rs` (`the_jet_classes_carry_their_disc_numbers`, `bf1_jet_borrowing_matches_the_bf2_twins`),
  `game/src/layout.rs` (the mission ceiling, in `bf1_mission_with_empty_roster_keeps_world_posts`).
- Bot rounds (`round_to_outcome 9000 42`, bf1_kas2 / bf2_kas2 / bf2_yav1 / bf2_cor1): outcomes and tickets
  unchanged; hashes move only by the hashed fuel of jet classes (a probe build without it reproduces main's).
- Owed outside the code: regenerate the local `engine/missions/*.ron` (`python3 scripts/gen_missions.py`) so they
  carry `max_player_fly_height`; until then every map's ceiling is the engine's 200.
- Not done here (card JETP2): the verifier and PCSX2 captures (step 9), bots (10), effect and sounds (11), the HUD
  fuel bar (12), `jetpack_hover` (13); the roster's route flag still says `JetJump > 0`.
