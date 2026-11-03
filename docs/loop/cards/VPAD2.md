---
id: VPAD2
title: A vehicle pad's class is one the mission script loaded: on 11 BF2 pad / team slots the port spawns a vehicle the PS2 never has (HOT1 an AT-ST at CP3, GEO1 3 fighter tanks + 2 STAPs, NAB2 GCW 4 combat speeders, Mos Eisley CW an AAT) - and the pads' post-load tick runs before the script re-owns posts (HOT1: a Rebel tauntaun at CP3 that decays in 20 s)
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
RE done 2026-10-07: `analysis/re-vehicle-pads-load.md` ([O], 419e1ec; verifier 40 / 2 / 0 of 42). Report
`reports/20261007_0421_card_VPAD2.md`.
- **The card's own symptom is no bug.** The PS2's AT-ST at HOT1's CP6 pad is not there at the start either: in the 0-AI
  side-select state the pad is armed with 16.17 s left of `SpawnTime` 45 and holds no vehicle. The port's battle spawns
  it at 45.03 s. The probe that saw nothing skipped `MapLayout::apply_mission` and stepped two ticks.
- **The bug the census found (part 1).** The PS2 looks a pad's class name up among the classes the mission script
  loaded and stores a null slot when none answers; the port resolves against the whole side file. The PS2's table, read
  from RAM in every kept state, against the port's: BF2 228 slots, **11 where the port spawns a vehicle the PS2 never
  has**; BF1 492 of 492 equal.

  | mission (port RON) | pad | port team | class the port must NOT spawn |
  |---|---|---|---|
  | bf2_hot1_con | vehiclespawn_6 | 0 (imp) | imp_walk_atst |
  | bf2_geo1_con | ift1, ift2, ift3 | 0 (rep) | rep_hover_fightertank |
  | bf2_geo1_con | cp5hailfire3, cp5hailfire4 | 1 (cis) | cis_hover_stap |
  | bf2_nab2_con | cp3_vspawn x2, cp1_vspawn, cp1_vspawn1 | 1 (all) | all_hover_combatspeeder |
  | tat2_con_cw | com_item_vehicle_spawn | 1 (cis) | cis_hover_aat |
- **The quirk (part 2).** Both games tick every pad once with dt = 0 in the post-load step, BF2 BEFORE
  `ScriptPostLoad` (`SetObjectTeam`). Only BF2 Hoth has pads on re-owned posts: `vehiclespawn_6` spawns a tauntaun for
  the world's owner (team 2) at once, CP3 then goes to the Empire, whose slot is null, and the tauntaun decays by the
  10 s class-changed rule (PS2 RAM at 28.8 s: last spawn team 2, armed 0, no vehicle). The port has no such tauntaun.

## Build
1. `scripts/gen_missions.py`: write every class a script's `ReadDataFile` calls list into the mission RON (a new field,
   `#[serde(default)]` in `MissionDef`; regenerate - every RON gains it). `vehicle_pads::build` / `install` take the
   mission's list: a class neither listed nor defined by the map's own `classes.ron` leaves the slot empty and goes
   into a new `PadBuild` list (not `absent`: that one means "no class of that name on the disc"). Without a mission
   (the plain `--map` path, tests) nothing is filtered.
2. `core::vehicle_pad` / `Sim::tick_vehicle_pads`: a pad's FIRST tick runs its spawn step under its load-time team
   (`PadDef::team`, the post's world team) and does not follow; the follow starts with the second tick. A pad whose
   post was not re-owned behaves as today (same tick, same team), so only `bf2_hot1`'s row should move.

## Acceptance (run and paste each)
- **Census referee, part 1:** rebuild the scratch example `engine/artifacts/lead_tmp/vpad2/examples/vpad2_census.rs`
  against the branch (it must pass the mission's list as the battle does), write its output over
  `lead_tmp/vpad2/port_census.txt` (keep today's as `port_census_before.txt`), run `census2.py` and `census1.py`:
  **BF2 `port_only` 11 -> 0 with `same` 228, BF1 `same` 492**, `ps2_only` 0 and `differ` 0 in both.
- **Pinned test:** `tests/vehicle_pads.rs` asserts the unloaded set is exactly the 11 slots above (data-gated like the
  rest of that test) and that `absent` is unchanged. Mutation: with the filter off the test fails.
- **Probe, part 2** (`examples/vpad2_probe.rs`, `bf2_hot1` + its mission, `0.1,19,22,30,50`): a tauntaun at
  `vehiclespawn_6` for port team 1 at 0.1 s, alive at 19 s, gone by 22 s and never replaced (no imp_walk_atst);
  `vehiclespawn_8` spawns imp_walk_atst_snow at 45 s (+ one tick); the other eight pads as before. PS2 reference: the
  two rows of §3's table. A unit test in `core` for the first-tick rule that fails on the parent.
  If the port's tauntaun lives to about 31 s instead, its `CP3_Control` box (centre -142.76, -15.78, 180.58, size
  64.54, 21.80, 50.89) does not hold the pad where the PS2's does (PS2: gone at 26.1 s and at 28.8 s, which only the
  10 s rule gives) - report the region test, do not tune the timer.
- **Gates:** sweep rows of bf2_geo1, bf2_hot1, bf2_nab2 move (declared); if the leak census rows of those three maps
  move, declare `leak` and say so; nothing else.

## Notes for the builder
- The port team of a PS2 team is team - 1; `bf2_geo1_con` is the CW mission (rep, cis), `bf2_nab2_con` the GCW one.
- Not in this card (listed in the RE doc §5, no card): seven pads follow a post-class object the port's layout lacks
  (BF2 HOT1 `shield`; BF1 HOT1 `G`, END1 `S`, GEO1 `A`) - they start alike and differ only once that object dies.
- The scratch worktree `<repo parent>/openbf-wt/vpad2-scratch` (08d6343) holds both examples built; its target
  is `lead_tmp/vpad2_target`.

## Original card (2026-09-29, RE phase)
PXWB (2026-09-29): in the 0-AI PS2 state an AT-ST stands parked at HOT1's CP6 pad (`engine/artifacts/pcsx2/ref/bf2_hot1/walk/pairs/cp1_cp6__sheet.png`
b180, left: yaw ~202 from cp6 = vehiclespawn_8 at 46 m, yaw 200.6). The port's pad install + 2 idle ticks
(`lead_tmp/vdraw/vdraw_pose.rs`: `setup_conquest(&layout, [0, 0], false)`, then `vehicle_pads::install`) reports 10
pads and 9 live vehicles; the missing one is this pad (`lead_tmp/pxwb/veh/bf2_hot1_port_vehicles.txt`). Its manifest
props carry ONE class key (1423113355 = "imp_walk_atst_snow") and ControlZone CP6.

### Original evidence / open [H]
- [H] VPAD picks the class per team side (Atk = team 1, Def = team 2) following the CP's owner; hot1g_con sets CP6 to
  team 1 (`game/src/mission.rs` test comment) — the pick may land on the side with no class key. Or vdraw's setup is
  not the game's (check with the real Sim / the walkabout's conquest setup).
- Every other matched pad on GEO1, UTA1, YAV1, END1, FEL1, HOT1 lines up (FINDINGS F51): xz within 8 cm, heading < 0.7°.

### Original acceptance (met by the RE: cause named, RAM reference, census)
- The cause named (the pad's props, the CP owner at spawn, the class pick) with the PS2 RAM's vehicle at that pad as the
  reference; if it is a port bug, a census of every BF2 pad whose class key is on one side only, and the fix.

## Planner note (2026-09-30, M-ALPHA)
`alpha: yes` — BF2 HOT1's CP6 AT-ST pad spawns nothing (a showcase map). Value 3 → 4. Showcase maps: BF1 NAB1, BF1 TAT1 (the port's `tat1`), BF1 YAV2, BF2 UTA1, BF2 MYG1, BF2 HOT1 (PLAN §Milestones M-ALPHA).

## Note (VREST RE, 2026-10-07): two more gates in BF2's spawn step
`analysis/re-vehicle-rest.md` §1: after its ground ray BF2's spawn step asks a box query at the pad
(`FUN_00161b50(pad+0x110, .., 1, 0, 8)`, B2:370349-370350) and retries in 1 s when it finds something; BF2's layer 8
is the hovers and walkers (verifier, from RAM), so a pad does not spawn while one stands in its box. (BF1 has no such
test.) Cards VREST and this one both edit `game/src/vehicle_pads.rs`.

- 2026-10-21 (card KNOCK's build, lane 2): **on BF2 Geonosis an empty pad hover (class 1) blows up with no destroyer and kills the bots beside it** - every attacker-less `blast` death of 16 traced seeds on both builds: 8 on main (9e381b1) and 16 on the KNOCK branch, each one an idle-decay or impact explosion of an empty class-1 hover at the same three spots ((-72, 278), (38, 296), (8, 111)) with soldiers 3-5 m off. Over 40 seeds the map's attacker-less blast deaths read 21 on main and 50 with KNOCK (the branch has fewer such explosions, 100 vs 109, with more bots near them; the push throws no living BF2 soldier, so how KNOCK raises the share is not shown). If class 1 is one of the fighter tanks this card removes, the fault goes with them; check the share after this card lands. Rows: `lead_tmp/knock/ab/trace/` (tb_* main, tr_* branch, `KT vexp` / `KT bdeath`; scratch print, `ab/tools/tracepatch.py`), counts `lead_tmp/knock/ab/{base_geo,branch_geo}`.
