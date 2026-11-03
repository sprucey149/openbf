---
id: STHOP
title: Soldiers walk along the floor's plane as the PS2's do: the grounded integrator follows the last contact's normal, contact is taken under the NEW feet within 2 cm, and stand falls after 1 s of airtime — not a level velocity, the OLD feet's floor, 0.05 m and a 0.5 m drop (KAM1 ramp: the port grounded 3 of 39 ticks, the PS2 20 of 24)
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## The RE answer (2026-09-30, `analysis/re-soldier-ground.md`, 9669e05; verifier 17/4/0 of 21)
The PS2 has **no snap**. What keeps a soldier on a ramp:
- **Plane-following.** While grounded, the integrator steers the velocity along the plane of the last floor contact
  (BF2 `FUN_002480a0`: T1 = right × N, T2 = N × T1; BF1 `FUN_002098b8`). Walking down a ramp has a real vy.
- **Contact after the move.** The grounded bit is cleared every tick and set again by the collision response for a
  contact on the 0.57 m foot sphere with n.y > 0.6 and approach < 0.1.
- **2 cm.** The push-out keeps up to 0.02 m of overlap, so small folds do not break contact.
- **Airborne:** N = up, gravity −18. BF2 stand/sprint → falling after 1.0 s, with full acceleration until then (BF1
  0.7 or 1.0 s, ×0.2).
- **Slopes:** BF2 ×0.1 control under n.y 0.7, uphill target × cos².

## Evidence (`engine/artifacts/lead_tmp/sthop/`)
- **PS2, BF2 KAM1, live RAM every update** (`ramp_bf2.py`, `kam1_ramp.csv`, `kam1_edge.csv`, `kam1_air.csv`). On the
  34.3° ramp:
  - grounded on every planar-facet update at 7.0 m/s along the plane (vy −3.9);
  - 4 of 24 updates without contact (the facet folds), the feet ≤ 0.028 m over the floor;
  - a 21-update flight off the rim, still in stand;
  - stand → falling one update after the timer reads 1.000.
- **Port main, same walk** (`port_kam1_ramp_main.txt`): grounded 3 of 39 ticks, in Fall 27, 0.14–0.85 m over the floor.
- **Prototype** (`proto_core.patch` on main b0f8fd3, scratch worktree `openbf-wt/sthop-scratch`; NOT gated, core's
  unit tests not run) — `port_kam1_ramp_proto.txt`: 22 of 24 grounded, none in Fall, the PS2's velocities to 2 decimals.
- **Sweep, 8 seeds × 32 maps** (`sweep4_main`, `sweep4_limits`, `sweep4_nolimit`, `cmp.py`):

  | | rounds | maps ≥ 7/8 | world deaths | KAM1 region | KAM1 noprog |
  |---|---|---|---|---|---|
  | main b0f8fd3 | 228/256 | 28/32 | 2557 | 1370 | 74 |
  | prototype (PS2 slope rules) | 227/256 | 28/32 | 1437 | 324 | 196 |
  | prototype, no slope rules | 220/256 | 26/32 | 1214 | 495 | 170 |

## Build spec (re-soldier-ground.md §3; the prototype does 1–4 for BF2's rules)
1. `Soldier` carries `ground_n` (up when airborne) and `air_time`.
2. Grounded integrator = BF2 `FUN_002480a0` (the port's handedness: T1 = N × right, T2 = T1 × N).
3. Move, wall pass, THEN the floor under the new feet (`Sim::ground_under_n`: the model ray's normal or the
   heightfield triangle's). Contact when feet ≤ floor + 0.02, n.y > 0.6 and approach < 0.1: feet = floor, N = its
   normal. Else airborne.
   - A too-steep floor under penetrating feet lifts them and takes the into-N velocity, without grounding.
4. Fall by airtime: stand/sprint 1.0 s, crouch stands up at 0.3 s (BF1: 0.7 / 1.0 s).
5. BF1's differences behind the per-game consts.

**Three traps the prototype hit:**
- probe from the feet only in the Jump stance (else GEO1 8/8 → 0/8);
- resolve after the wall pass (else units sink through railed edges);
- compare sweeps on ONE base (STFAL landed mid-session and made the first comparison read +50 % deaths).

## Landed 2026-09-30 (main eb90b56; report `reports/20260930_2055_card_STHOP.md`)
- Probe walk on the landed code: 24 ramp ticks, 22 grounded, 0 in Fall. PASS.
- Sweep seeds 1-8 on one base (`lead_tmp/sthop/sweep5_main` vs `sweep8_final`): rounds 228 -> 226 of 256, maps >= 7/8
  28 -> 28, world deaths 2557 -> 1468 (57 %). PASS (rounds at the limit).
- KAM1 world deaths 367 of 497 (74 %, was 96 %): NOT <= 5 % - carried to STLVL.
- BES1's falls on the PS2: NOT measured - carried to SLAND.
- Beyond the spec: `ai::steer::steep_ahead`, the swept floor probe (`ground_under_n(.., from_y)`), `WEDGE_MAX_DY`;
  leak census moved and was declared (push 781 -> 923, tunnel 1441 -> 1641, float 67 -> 193, runs in all -923).

## Not in this card
- **The sphere foot.** The PS2's feet ride +0.02…+0.22 m on slopes, roll over crests and bridge collision cracks; the
  port's point foot drops one tick through the crack at KAM1 (−44.6, 286.3). A sphere-cast foot is a later
  refinement.
- **Landing damage** → card SLAND.
- **The wall pass's steep-face push** (the PS2 pushes along the true normal for 0 ≤ n.y ≤ 0.6) — noted on SLAND.

## Original Why (2026-09-30, STFAL)
STFAL's fall traces (`lead_tmp/stfal/kam1_fall_s1.err`, `kam1_v4_fall_s{1,2,3}.err`, `classify.py`). On BF2 KAM1
a soldier walking down a ramp (e.g. (−38.8, 56.5, 284.3) → the 51.4 deck, floor dropping 0.15 m a tick at
6 m/s) goes airborne: `Soldier::step` compares the new y with the ground sampled at the OLD feet
(`sim/step.rs` `ground_under(pos)` before the step) and flips `grounded` off past 0.05 m. It then falls under gravity
with the in-air control factor (0.02–0.03), lands every ~17 ticks 3.4 m further on, and if the ramp's straight line
leaves the walkway it lands in the sea. The steering (STFAL) cannot act mid-air.
- seed 1, main 7b8e3b0 + STFAL v1: 45 of 76 KAM1 falls airborne > 5 ticks at the edge.
- STFAL (de4ba72), seeds 1–3: 273 of 456.
This moves the player too: walking down any steep ramp hops.
- Planner 2026-09-30 17:30: alpha: yes — the ramp hop moves the player on every map, not only the bots on KAM1.

## Note (KAMR RE, 2026-09-30, lane 3): a PS2 reference trace, and two more faces of the same ground query
- PS2, BF2 KAM1 (`lead_tmp/kamr/ps2exp/out/rail_0.csv`, 20 Hz, feet from RAM E+0x150; `kamr_ps2.py rail`): the soldier
  walks off cp5's pad — over the 0.57 m rail (feet 78.42 -> 79.11) and then DOWN THE HULL GLUED to it: r 27.8 -> 75.82,
  31.1 -> 73.58, 34.0 -> 71.61, 37.3 -> 69.38 (slope 0.68 = 34 deg, y linear in r, no hop), off the rim and dead.
  The port on the same line (`lead_tmp/kamr/tools/kamr_walk.rs bf2_kam1 21.86,-70 21.65,-45`) is airborne from tick 60
  and lands every ~18 ticks.
- The same query falls through slits between collision floor pieces and hides a walkable face under a steep one: card
  GRND (deps STHOP — one ground rule, not two). `analysis/re-kamino.md` §4.
