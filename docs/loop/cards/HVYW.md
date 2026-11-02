---
id: HVYW
title: Heavy weapons with a purpose: a bot with a rocket launcher (and a super battle droid's wrist rocket) fires it at enemy VEHICLES in range and line and at GROUPS of 3+ within 9 m, and otherwise keeps to its rifle (the maintainer's INBOX 2026-10-12 16:20, ask 1; deviations approved) - today the weapon choice is for soldier targets only and a fight never takes a mounted target
milestone: M3
value: 5
phase: build
status: done
deps: BOTD
files: engine/crates/core/src/ai/arsenal.rs,engine/crates/core/src/ai/behave.rs,engine/crates/core/src/ai/gunner_foot.rs,engine/crates/core/src/ai/state.rs,engine/crates/core/src/perception.rs,engine/crates/core/src/weapon.rs,engine/crates/game/src/roster.rs,engine/crates/game/examples/round_to_outcome.rs
declares: sweep; arena
needs:
effort: high
branch: hvyw
resume: landed 3fba385b 2026-10-15
created: 2026-10-12
alpha: yes
---
## Why
The maintainer's wish (INBOX 2026-10-12 16:20, ask 1 - read it first; under `## Done` now): heavy weapons with a purpose. A bot
with a rocket launcher, and a super battle droid with wrist rockets, fires them at VEHICLES and at GROUPS, and
otherwise keeps to its rifle. Deviations from the PS2 that make bots play better are approved (his 16:00 note).

## Evidence (read from the code on main 5fbbbd4, not measured)
- `ai/arsenal.rs` `choose` (line 134) walks every slot by ammunition, the target-kind mask, the range band, "plenty"
  and a score - for SOLDIER targets only; its header: 'Not built: the "too weak for a vehicle" rule'.
- `ai/behave.rs:480`: a fight's target filter is `e.mount.is_none()` - a mounted enemy (a vehicle's crew) is never a
  foot bot's fight target, so nothing sends a rocket at a vehicle.
- The grenade alone has a group test (9 m group, a 3 s window in 30 s: card GRAI / GRAIA, `arsenal.rs`).
- Not measured yet: how many rockets a round's bots fire today and at what (the census below is step 1, RULES 2).
- The PS2's side: `analysis/re-soldier-ai.md` (weapon choice, the target-kind mask, 7 and 10a for vehicles as
  targets) - read what the PS2 does with a vehicle target and the "too weak" rule FIRST; where it already has the
  rule, build the PS2's and name only the rest as ours.

## Notes
- 2026-10-12 16:58 planner: filed from the maintainer's note. `deps: BOTD` only - it does not need TEAM. Rules 2-4 deviate from
  the PS2 only where the RE says so; name each in the code and in FEEL_NOTES.md.
- Files: `ai/behave.rs` is shared with most open bot cards (RLOOP, RETW, TPOS, TEAM, TURR, CLSE2B): keep the edit there
  to the target filter; the choice itself lives in `arsenal.rs`.
- A vehicle's gunners and turrets shooting at foot soldiers are not this card (VTAT, TURR).
- 2026-10-12 21:35 lane 1 (build, cut by STOP at 21:13; branch `hvyw` 8392703 WIP, docs ce47902 = re-soldier-ai 22):
  - **Census of main fb4a2f1** (8 seeds x 32 maps, `lead_tmp/hvyw/sw/main`, `tools/hvcen.py sw/main --maps`): rocket +
    torpedo launchers in hand 215,600 s, 147 shots, 0 at a vehicle; the wrist rocket never drawn; other weapons fire
    106,655 shots at riders; vehicles destroyed: nobody 2,083, a rider 88, a bot on foot 191 (3 with a heavy weapon).
  - **Cause** (read, then confirmed by the build): `arsenal::think` asked the choice with the RIDER's kind (a person),
    so the launcher's mask (`AITargetPerson 0`, `AITargetVehicle 1` on the disc) refused every vehicle. The PS2 already
    has the vehicle rule and the "too weak" rule (re-soldier-ai 22, both games, one verifier): built as the PS2's.
    Ours are only: the crowd of 3 (BF2's own exemption wants 5, BF1 has none), the wrist rocket against vehicles (its
    mask says no), the target preference, the reload kept.
  - **Build 1** (one 8 x 32 block, `sw/full1`; every rule off = main byte for byte, `sw/null1`): heavy shots by NAME
    1,742 -> 11,787; at a vehicle 342 -> 5,813; at a group 221 -> 4,166; at a single soldier 1,173 -> 1,658 (14.1 %:
    OVER the 10 % bar - 1,248 of them are three person weapons the name catches, BF1's jet trooper launcher, the
    bulldog and the EMP launcher; the launchers' own 410 were mostly "nothing else in reach, fire what is in the
    hands", closed in build 2 by `ArmState::spare`, NOT yet swept); vehicle kills by a heavy weapon on foot 3 -> 142;
    256 / 256; recycles 48 -> 55; team + self kills 717 -> 752 (1.28 -> 1.32 %), 111 by launchers (was 3): bf2_fel1
    1.21 -> 2.13 %, bf1_hot1 0.20 -> 1.34 %, bf1_yav2 1.54 -> 2.01 %, worst map tan1 2.80 (main 3.11).
  - A missile is a straight bolt with no blast in the port (card UORD, value 3): a rocket at a group hits ONE man.
    Planner: UORD's missile part is worth pulling forward now that rockets are fired.
  - A latent hang met in a test fixture: `WeaponState::fire_cycle` never ends for a weapon with ShotDelay 0 that is
    not TriggerSingle and whose clip is empty (refire time 0, `take` 0). No disc class of the roster is such (the
    launchers are TriggerSingle); worth a guard.
  - Tools: `lead_tmp/hvyw/tools` (sw.sh with the STOP check, hvcen.py, wdump.py); verifier report
    `lead_tmp/hvyw/verify/verifier_report.md`.
- 2026-10-13 05:02 lane 1 (resume; branch `hvyw` 548ef04 on bea783c, ci green, NOT landed: the fidelity gate refuses on one all-map
  value, sorted NOISE - report `reports/20261013_0505_resume_HVYW.md`):
  - **The session before this one** (died at 00:12 on the limit, no report) had committed bea783c (build 5: the line-of-fire rule
    for missiles, team kills by launchers 111 -> 38) and met the gate: `stuck_max_med` 166.88 -> 185.62 and `world_death_pct`
    1.08 -> 1.48 (`gates_fid/`).
  - **Standing at a vehicle** (read first, as the refusal asks): spells of 20 s or more standing with a rider as the target - main
    8 (201 s, 3 without a shot), build 5 46 (2,272 s, 42 without a shot). Three causes, each switched alone (`sw/b6`, `a6`, `full6`,
    `s7`, `full8`): riflemen sparing a walker on their own post (now: only past 10 m), a fight position for a weapon out of its
    band or a grenade (now: no), and a bot held still for a grenade throw at a vehicle that is refused every tick (now: it walks
    on) - six units traced (`trace/`). Final: 6 spells (160 s, 4 without a shot: boarders and capturers).
  - **The census, main fb4a2f1 beside 548ef04** (8 seeds x 32 maps, `sw/main`, `sw/clean9`; `tools/hvcen.py`): launchers' shots
    147 -> 8,823 - at a vehicle 4,937 (56.0 %), a group of 3+ 3,439 (39.0 %), a single soldier 349 (4.0 %; bar 10), no target 98;
    vehicles destroyed by a bot on foot 191 -> 289 (114 by a heavy weapon); team + self kills 717 -> 643 (1.28 -> 1.15 %; the
    gate's value 1.31 -> 1.20, worst map Tantive IV 3.32 -> 3.48, largest rise Death Star 1.62 -> 2.12); kills a minute 34.70 ->
    34.52; 1,536 / 1,536 rounds over six blocks.
  - **The refusal** (`gates_fid2/`, `gates_auto/`): `stuck_max_med` 166.88 -> 187.50 alone. NOISE: six blocks of 8 seeds each
    (`tools/fidsw.py`): main 166.9 / 183.8 / 181.9 / 161.3 / 183.8 / 174.4 (two of main's own blocks are over the gate's 181.9),
    branch 187.5 / 172.5 / 178.1 / 176.3 / 189.4 / 183.8; paired +5.9, t 1.2. Inside it one real piece: BF1 Yavin 4 (block medians
    220 -> 320, recycles 12 -> 20 in 48 rounds, at main's own hub-18 pit) and BF1 Endor (bots inside two trees, 2 -> 11: card ETREE's
    note of today).
  - **`world_death_pct`** passes now by 0.0005 (1.4752 against 1.0757 + 0.4) and is a real move: +0.29 over six blocks, all of it
    riders of destroyed vehicles (421 / 445 / 473 -> 574 / 596 / 622; other world deaths 116 / 111 / 115 -> 143 / 103 / 112;
    `tools/wcause.py`). The port counts a rider's death to the world whoever destroyed the vehicle: card RIDK.
  - **Where bots die** (`contact_spells.py`, 17 BF2-rule maps, per-seed pairs): no enemy within 20 m at death 41.4 -> 43.3 % (6 of 8
    seeds, t 2.9) - the card's bar "not farther out" is NOT met by 1.9 points; an enemy inside 10 m 29.0 -> 28.6 % and contact
    12.18 -> 12.26 % (both flat), the median 17 m on both. What it is (`tools/fardeaths.py`, `riderkills.py`): rockets at crowds
    kill from range (kills with the launchers under 100 -> 1,437 over all maps) and riders live longer (killed while mounted 2,031 ->
    1,346; kills by mounted units 1,821 -> 2,025).
  - **The maintainer's Kamino scene** (8 rounds, 40 snapshots): trails over 6 reversals 8 of 431 on main, 0 of 408 here.
  - **The wrist rocket is in nobody's hands** (slot 2, channel 1, not thrown: `roster::held_slots` drops it; no census row names
    one): card WRST. The choice's rule for it is tested on a fixture only.
  - Tests: 3 new scenes; 11 mutations, 10 caught (`mut/report2.txt`); the survivor widens the no-stand rule to soldier targets.
- 2026-10-13 05:13 planner: the NOISE table reproduced and confirmed on 96 fresh seeds; NOT parked - the same run found a pile at BF1 Endor's cp2 boulder on the branch only (27 recycles in 4 of 144 rounds against main's 2 in 2; seed 91: both teams' living units there for 500 s without a kill). `resume:` has the numbers and the next step; report `reports/` planner 2026-10-13. Undo: `loopctl.py set HVYW status=blocked` with the old resume (`lead_tmp/planner_1013a/HVYW.before.md`).
- 2026-10-13 05:46 lane 1 (resume; branch `hvyw` 1f79720 = 548ef04 + one test, ci green 1469 / 0 / 44; NOT landed; report
  `reports/20261013_0547_resume_HVYW.md`). The planner's four steps:
  - **(a) Why nobody fights at the boulder, and whose fault: MAIN's - now card VMESH (value 5).** The boulder is a closed
    collision mesh with the floor inside it; a vehicle gets in (a bike through the wall at 23 m/s, an AT-ST's centre into the
    edge), `Sim::exit_point` puts its rider down inside, and no sight ray crosses the hull. On MAIN's binary, seed 74
    (`lead_tmp/hvyw/boulder/main_s74_{entry,rides,vehicle,pile}.txt`): the last Imperial inside, 15 of 16 Rebels within 14 m,
    0 of 1,922 unit rows with a target in 60 s, the last kill 100.8 s before, stalls to 106 s - ended by his recycle at 120 s.
    Seed 91 on the branch is that with two Rebels inside whose stall watch a `capture` tick closes (806 s).
  - **(b) What the card adds, measured** (`boulder/inboulder_288.txt`, BF1 Endor seeds 1-288, 4 Hz rows of whole rounds; a stay =
    5 s or more on foot inside the mesh): rounds with a stay 12 on main, 16 on the branch (the door opens as often); stays 20
    -> 111, unit-seconds inside 1,859 -> 9,159, recycles at the boulder 8 -> 41, rounds over 2 recycles 0 -> 4 (seeds 73, 91,
    189, 236). All 131 stays began with a dismount. Which rule (`inboulder_rules.txt`, build 4's switches, seeds 1-144): all
    off 6 stays / 593 s (= main's six rounds), the vehicle rule off 10 / 642, all on 19 / 2,326. How (`doorfeed_288.txt`):
    in the 60 s after a first exit inside, enemies on foot within 40 m 1.74 -> 0.28 a door and the riders' own side on foot
    within 30 m 0.79 -> 1.56 - the side that holds cp2 stands there unchallenged and boards the vehicle in the mesh again
    before its idle decay (exits inside 1.7 -> 7.1 a door). The card's diff touches none of the path (seats.rs, the hover,
    ride.rs, stall.rs, mover.rs: `git diff --stat main...hvyw`).
  - **(c) "No round over 2 recycles at the boulder" is NOT met on the branch** (4 of 288; main 0) and cannot be without VMESH:
    the bar is VMESH's acceptance now, with this branch's binary.
  - **(d) The surviving mutation is pinned** (1f79720): `a_bot_stands_for_a_throw_at_soldiers_from_the_tick_it_is_picked` -
    passes, fails under the mutation ("12 m: 3.836687 m/s with the grenade picked"), passes restored.
  - Tools: `lead_tmp/hvyw/tools/` boulder_mesh.py (the mesh in world space, ray parity), inboulder.py, wedged.py, vehrows.py,
    doorfeed.py, bsnap.sh, endbq.sh / endbq2.sh; runs in `lead_tmp/hvyw/boulder/` (e_main, e_main2, e_br9, e_br92, e4_*).
- 2026-10-13 07:09 planner (lane 2): **the Endor claim is judged EXPOSED** (a fresh verifier told to refute; `lead_tmp/planner_1013b/verdict.md`,
  my reproduction `inboulder_repro.txt`): (a) stands - main's binary (fb4a2f1 + the read-only census observer, its Endor rows equal
  the all-rules-off build byte for byte); (b) stands narrowly - rounds with a stay 12 / 16 is chance (p 0.56), exits a door 1.7 / 7.1
  is not (median 1 against 3, p 0.005; without seeds 91 and 236 p 0.012); (c) stands by reading - no hunk in the path, and the four
  hunks beside it need a seen target, which nobody in either pile has (36,755 unit rows on the branch's seed 91, 0 with a target).
  **Corrections to the 05:46 note:** 65 of the branch's 114 exits are seeds 91 and 236, and those are a WALKER standing at the
  boulder's south-west corner that is boarded by pair after pair - a loop main also has (seed 227, 19 boards, the seat 2 m outside
  the mesh); "enemies on foot 1.74 -> 0.28" does not hold (0.63 / 0.71 over the time a door is open) and "it comes with the vehicle
  rule" is build 4's, not the measured build's (scatter as large as the effect). No approval is written: there is no per-map gate
  token, and the gate's own refusal is an all-map value - the maintainer's (`needs:`, STATUS question 45). Undo: `lead_tmp/planner_1013b/HVYW.before.md`.
- 2026-10-15 10:40 planner (lane 1): the maintainer answered YES to STATUS 45 and 46 (INBOX 2026-10-13). `[approvals.HVYW]` written in gates.toml:
  `fidelity = "stuck_max_med@agg,world_death_pct@agg"` (the two all-map values his yes names; the boulder pile is owed to VMESH as a
  comment, not an `exposes` token - the gate voids that on an agg value). `needs` cleared, `status=parked`: the next land job takes it.
- 2026-10-15 11:07 lane 2 (land): LANDED 3fba385 (rebased on 6dc59d4; a CRATES.md count conflict resolved by hand, counts commit 3fba385).
  Census on the landed binary 8 x 32 = build's `sw/clean9` in every row (heavy shots 8,823: vehicle 56.0 %, group 39.0 %, single 4.0 %;
  foot vehicle kills 191 -> 289; team + self 1.15 %; 256 / 256). Fidelity: stuck_max_med agg alone, approved. Report
  `reports/20261015_1106_land_HVYW.md`. Owed: remove [approvals.HVYW] (planner), VMESH, RIDK, WRST.
