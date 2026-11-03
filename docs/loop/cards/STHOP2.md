---
id: STHOP2
title: Measure STHOP's remaining open-face grounding, steep-contact velocity and crouch airtime; compare from-rest ramp trials while preserving SPUP's approved uphill sprint
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
Found by cleanse CLNC's review of STHOP (81aae53 / de3b9c1 / eb90b56), each reproduced with a scratch test on main 5043dc3 + comments (report `reports/20260930_2125_cleanse_core.md`). All four are movement a player feels on any map with ramps or cliffs.

## Evidence (port numbers measured 2026-09-30; scratch test = Soldier::step + resolve_ground on an analytic plane, dt 1/30)
1. **Wedged on an open face.** `resolve_ground`'s wedge test asks only whether the feet's height changed by < `WEDGE_MAX_DY` (5 mm) over the tick. A soldier put at rest on an OPEN plane of n.y 0.6 and of n.y 0.3 (72 deg) is grounded on 90 of 90 ticks, vel 0, stance Stand, in both games — he stands on a cliff with full control (spawn, the separate_units y-snap, a vehicle exit, the top of a slide; walking along a contour passes the same test). The PS2 stands him only when his steep contacts' MEAN normal has y > 0.6 (re-soldier-ground.md 1.4) [D].
2. **Sprint on a ramp.** The sprint gate (`planar_speed >= 0.9 * max_speed`, soldier.rs) and the roll impulse read XZ speed; since STHOP the speed is along the plane. Holding forward + sprint from rest, 120 ticks: BF2 rules UPHILL never sprints at n.y 0.95 (XZ 7.72, 3-D 8.12, gate 8.10), 0.90, 0.826, 0.75; downhill sprints at tick 4 on all. BF1 rules: n.y 0.90 never sprints either way (XZ 7.29, 3-D 8.10); 0.95 sprints. re-stance-roll.md says the PS2 compares 'actual speed' — which speed, and whether the PS2's own uphill scaling also blocks a sprint, is unmeasured.
3. **Steep-face contact.** The port removes the whole into-face velocity on a face of n.y <= 0.6; the PS2's wall contact flattens the velocity's copy of the normal (only horizontal velocity removed) and lifts by the true normal (re-soldier-ground.md 1.3). With BF2's full air control the port turns stick speed into climb speed up the face [inferred, not measured].
4. **Crouch airborne** stands up at 0.3 s and then falls at air_time > 1.0 s in total; BF2's Stand case may zero its timer on its first airborne update (1.3 s total) — the latch after crouch is unread [H].

## Historical acceptance (2026-09-30; sprint scope corrected below)
- PS2 numbers for each (BF2: `lead_tmp/sthop/ramp_bf2.py` sampler + the RAM hold of `pcsx2/walk_bf2.py`; BF1: `ps2live.py`): (1) the player released at rest on an open face of n.y < 0.6 — grounded bit and speed after 1 s; (2) forward + sprint from rest up and down KAM1's ramp (n.y 0.826) and one ~18 deg ramp — the tick the sprint stance starts, or never; (3) stick held into a ~55 deg face for 2 s — height gained; (4) step off an edge crouched — time to the Fall stance.
- Then build: the same four scratch runs on the port match the PS2 (grounded yes/no; sprint tick within 3; height within 0.3 m; fall time within 2 ticks), as unit tests in soldier.rs; sweep_seeds.py finishes not below main's; the leak census read per class (float / floor / push) before the gates.

- Planner 2026-10-02 14:45: `files:` emptied while the phase is `measure` (the PS2 numbers come first; the list held `sim/step.rs`). Restore when you set phase=build: `engine/crates/core/src/soldier.rs,engine/crates/core/src/sim/step.rs` (JARC and BF1BK edit `soldier.rs` too: one at a time).

## Acceptance (current; planner reconciliation 2026-10-24)
- Cases 1, 3 and 4 above remain required in both games: raw PS2 per-update rows for
  grounded bit/speed after 1 s released on an open face below n.y=0.6, height after
  2 s into a roughly 55-degree face, and crouched edge-to-Fall time. Port scratch
  runs must match grounded yes/no, height within 0.3 m and Fall time within 2 ticks.
- Case 2 still requires BF2 from-rest forward + sprint uphill/downhill trials on
  KAM1's n.y=0.826 ramp and an approximately 18-degree ramp, with the sprint-entry
  tick or never printed. Also retain a flat-floor control (entry within 3 ticks of
  the PS2) and the 3-tick tolerance wherever no approved SPUP deviation applies,
  including downhill trials. SPUP's historical L3-during-run rows do not replace
  these trials.
- On runnable slopes the port intentionally permits uphill sprint (SPUP,
  `re-stance-roll.md` 4.2b and `FEEL_NOTES.md`). Print the PS2/port difference;
  preserve SPUP's sprint-start limit of 1 s on its 10/20/30-degree runnable
  slopes under both port GroundRules, and retain
  `a_sprint_starts_while_running_uphill` and
  `the_uphill_run_is_the_ps2s`. A mutation restoring the old horizontal-speed
  threshold must fail the uphill referee. Do not undo the approved deviation
  to match a PS2 never-sprints row. The historical 120-tick run is an observation
  window, not a relaxed sprint-start limit.
- Authentic BF1 trials use walk/run for the slope comparison; BF1 has no PS2
  sprint. The port's BF1-content sprint is checked as its modern movement rule,
  separately from authentic BF1 parity. No BF1 sprint tick may be invented.
- Before a build, record the remaining measured changes and restore file
  ownership as above. Keep the original sweep-not-below-main requirement,
  float/floor/push leak census, full CI and required gates. No check ran here.

## Cloud prerequisite checkpoint (2026-10-24 08:41 UTC)
- No PS2 trial or port acceptance run performed. Exact 30-path inventory: `lead_tmp/sthop2_20261024_0840/prerequisites.json` (25 missing); report `reports/20261024_0841_card_STHOP2.md`. Phase remains measure; original acceptance retained pending evidence reconciliation.
- Item 2 describes September code. SPUP (done, historical c1ae263) subsequently measured the BF2 forward-row velocity test and implemented the approved uphill sprint deviation. Current `Soldier::sprint_start_speed`, `soldier/tests.rs` (`a_sprint_starts_while_running_uphill`, `the_uphill_run_is_the_ps2s`), `re-stance-roll.md` 4.2b and `FEEL_NOTES.md` record it. Preserve that deviation; do not restore the obsolete XZ gate to satisfy literal parity. Historical SPUP rows use L3 pressed during a run, so they do not supply this card's requested from-rest uphill/downhill sprint ticks on n.y=0.826. BF1 PS2 has no sprint; distinguish the authentic BF1 trial from port BF1-content rules when reconciling acceptance.
- Remaining open-face/contact/crouch claims stay unmeasured here. Restore PCSX2 README/session tools, BF2 ramp sampler and RAM hold, BF1 ps2live, original local disc/reference data and raw trial rows before continuing. Missing bakes and GPU also prevent later sweep/leak/golden checks. No source change or current passing receipt.
