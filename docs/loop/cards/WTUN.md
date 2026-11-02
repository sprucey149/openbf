---
id: WTUN
title: Soldiers walk through walls with no push (LEAK census class tunnel: 2k-12k runs per map): the soldier's axis crosses a SOLDIER-layer wall face while the wall pass pushes < 0.25 m — find why push_out lets it (suspect [H]: push_out keeps only each mesh body's DEEPEST triangle contact and drops the body when that contact's normal is sloped, |n.y| >= 0.5, so a wall in the same body gets no push), fix, and drive the census's tunnel count down
milestone: M3
value: 5
phase: build
status: done
deps: LEAK,JWAL
files:
declares: leak; sweep; arena; sim
needs:
branch: wtun
resume: landed 96d61731 2026-09-29
created: 2026-09-29
---
## Why
The LEAK census (game/examples/leak_census.rs, report reports/20260929_*_card_LEAK.md) drives a real soldier at every soldier-layer wall of all 32 maps. Class `tunnel`: between two ticks a point of the soldier's axis above the step (0.8/1.25/1.7 m) crossed a wall face (|n.y| < 0.5) while the wall pass pushed less than 0.25 m. The axis sits inside the 0.5 m capsule, so a legal move never crosses a face: every one is a soldier walking into a wall.

## Evidence
- Counts per map: `leak_census` stdout (baseline leak_main_1.txt), e.g. bf2_dag1 12,460, bf2_fel1 10,687, bf2_nab2 8,553, bf1_nab1 1,719 runs. Sites: `LEAK_SITES=<file>`.
- A traced case: bf1_nab1, `LEAK_TRACE=-134.49,73.23,walk`: walking at 6.3 m/s, tick 2 the axis crosses a face with normal (0.77, 0.11, -0.63), push (0, 0) every tick.
- core/src/collision/walls.rs push_out: per body ONE contact (mesh_contact keeps the deepest triangle), then `if n.y.abs() >= WALKABLE_MIN_NY { return; }` — the whole body is skipped [V, code]. That this is THE mechanism is [H].

## Notes (JWAL's RE, 2026-09-29, lane 2 — analysis/re-soldier-walls.md)
- This card's suspect is JWAL's M1, proven there to the tick. `push_out` skips a contact whose normal has
  |n.y| ≥ 0.5 as "a floor". The skipped contact is a ledge's rim, or a mesh body's deepest triangle when that is a
  floor while a wall triangle also penetrates.
- JWAL's build fixes it for primitives and meshes: a floor-like stepped contact is a ledge, re-asked with the full
  capsule over WALL-face triangles. So this card now waits on JWAL (deps).
- After JWAL lands, re-run the census. What is left of `tunnel` is this card's residue: trace it, card it, or close
  this card if nothing is left.
## Note from WPSH (2026-09-29)
The suspect is confirmed at the cp5 hut (`lead_tmp/wpsh/hut_contacts.txt`, `analysis/re-soldier-walls.md` §4.2).
With WPSH's far-side rule dropping triangle 5, body 386's deepest remaining contact is triangle 13's, which is
ceiling-like (n2.y −0.53). push_out skips the whole body, so triangle 18's outward wall contact (0.456 m, normal
(−0.303, 0, 0.953)) never pushes, and the axis crosses unpushed. The census counts that as `tunnel`. Probe:
`lead_tmp/wpsh/tools/wpsh_probe.rs` (a scratch example; add parry3d as a game dev-dependency to build it).
## Note from the JWAL audit (2026-09-29, `reports/20260929_2240_audit_JWAL.md`)
- The cp5 hut control still gets in on main.
  - `LEAK_TRACE=1.28,4.66,jump` crosses triangle 5 unpushed at tick 20. The face's n.y is −0.53, so the census files
    the run as `ceiling`, not `tunnel` (leak_census.rs: `|n.y| >= WALKABLE_MIN_NY && n.y < 0`).
  - At tick 21 the soldier moves 0.97 m more than his velocity explains (a mostly sideways push).
  - He ends inside the ring: r 6.66 m from (4.886, −7.062), still sliding; the wall stands at 6.8–7.5 m. See
    `lead_tmp/audit-jwal/hut_replay.txt` and `hut_trace_main.txt`.
- So closing WTUN's skip should also remove a `ceiling` run, not only `tunnel` runs.
- For the PS2, see SRAD. Its handler (`FUN_00202f38`) has no 'floor, so skip' rule for the soldier's body spheres:
  a lower- or upper-sphere contact is classified (wall / ground / flattened) and pushed. The only zeroed pushes are
  projected depths over 2 m.
## Outcome (2026-09-29, lane 1)
Landed a01d740/96d6173 (report `reports/20260929_2337_card_WTUN.md`, analysis/re-soldier-walls.md §4.3). Two
mechanisms: a mesh body's deeper overhang hid its wall (the wall contact now wins); a leaning primitive's lower edge read as
a ceiling (a ceiling contact now pushes by its horizontal part). Census tunnel 9,587 -> 1,427; the residue is card WTHIN.
