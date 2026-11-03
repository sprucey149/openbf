---
id: VTZT
title: VTURZ's tests miss four mutations (audit VTURZ): the building's hidden range is checked against itself, R3 is tested only as a one-tick tap, the zoomed FOV's expectation is read from the parsed field, and no test drives the game loops' per-viewer draw list; plus three doc numbers to correct
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
Audit VTURZ (2026-10-16, `reports/20261016_0115_audit_VTURZ.md`): the 6 mutations the test file names are all killed, but four new ones survive with every test green (logs `lead_tmp/audit-vturz/m{1,2,3,4}_*.log`, worktrees `openbf-wt/audit-vturz-m{1..4}-scratch`):
- M1 `scene.rs` TurretDraw::new's `whole_start..draws.len()` -> `(whole_start + 1)..draws.len()` (a zoomed gunner sees one draw of his building): the test takes `mine` from the same `whole` field.
- M2 `seats.rs` ride's zoom press without the `prev_buttons` edge test (holding R3 toggles every tick): `tap()` holds one tick.
- M3 `vehicle_class.rs` FirstPersonFOV branch disabled: the FOV check derives `want` from `first_person_fov.unwrap_or(48)`; no zooming turret class on either disc authors the key (0 of 44, `lead_tmp/audit-vturz/fov_census.txt`), so the rule is the 48 default in practice.
- M4 `headless.rs` / `windowed.rs` back to `gpu.draws.clone()`: no test runs the frame's draw list (418 game tests pass).
Doc numbers: `analysis/re-turret-draw.md:327` and the commit say the yaw-only eye is "up to 2.6 m off" - it is 3.23 m (2.59 m is only the height gap; `yawonly_mutant.log`); line 328 and the test's comment call myg1 row 30 a lagging first zoomed update - rows 30-31 have the zoom flag 0, the flag and the zoomed camera start together at row 32 (`replay_all.log`); `guns.rs` tick_seat_zoom says BF1 has "three callers of FUN_00283c18" - the ELF has 4 jal sites (the 4th in FUN_0032db40, none in the turret's FUN_00223850; receipt ba8ba81c).

## Acceptance
Each of M1-M4 (the exact edits above) fails at least one test; the 6 named mutations stay killed; behaviour-neutral (every gate PASS); the three doc lines corrected.
## GitHub checkpoint recovery (2026-10-24)
