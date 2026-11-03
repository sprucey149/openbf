---
id: NEARP
title: The port's near plane is 0.1 m in every view, the PS2's 0.5 m (zoom-scaled [H]): zoomed in BF2 Mygeeto's AA gun (tur_bldg_recoilless_lg1) the scope eye stands 0.26 m behind a face of myg1_bldg_rock_face1 and the screen goes flat grey / black where the PS2 shows the ground (audit VTURZ)
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
Audit VTURZ (2026-10-16, report `reports/20261016_0115_audit_VTURZ.md`): the landed turret zoom puts the camera exactly where the PS2's stands (124 of 124 zoomed RAM rows within 0.03 mm), but on BF2 Mygeeto - a showcase map - two zoomed looks of the AA gun at (-118.16, 0, 302.01) show a flat grey / black screen. The eye is 0.26 m behind a back face of `myg1_bldg_rock_face1` (-120.585, -0.646, 312.987); the port's near plane is 0.1 m (`render/src/camera.rs:39`) and every draw is two-sided (`pipelines.rs:707`), so the inside of the rock fills the view. The PS2's near plane is 0.5 m (`analysis/re-sky-pass.md:218`); `analysis/re-weather.md:146-151` reads near = camera +0x130 and a zoomed frustum from near x zoom [H for the projection]. The maintainer would see it within a minute in the AA gun.

## Evidence
- PS2 `lead_tmp/vturd/ps2/myg1_zoom_3_zoomright.png` (ground with tread marks) | port at near 0.1 (flat grey, std 1.4) | port with that one draw dropped or at near 0.3 / 0.5 (the same ground): `lead_tmp/audit-vturz/pairs/grey_triple.png`; `grey_probe.py myg1_zoom 3_zoomright AUDIT_NEAR_PLANE=<m>` (needs the scratch pose_render of `scratch_main.diff`).
- Ray test: `lead_tmp/audit-vturz/rock_ray.py` (the centre ray's only hit is a back face at 0.26 m).
- Real game loop (headless, `--ai 0`, the scratch `AUDIT_SEAT` hook in `scratch_main.diff`, input `zoom_walk2.ron`): tick 228, look (-23, 0) black at near 0.1, the base at near 0.5 (`realloop_tick228_near.png`); of 45 frames of the sweep only that one depends on the near plane.
- Other pairs: Mygeeto 2_zoom / 4_zoomup unchanged by 0.1 -> 1.5 m; Naboo 7_zoom / 8_zoomright unchanged at 0.5, changed at 1.5 (foliage within 1.5 m) - the pair that settles fixed 0.5 against 0.5 x zoom (zoom 2 there).

## Acceptance
1. Read the PS2's near (camera +0x130) in a zoomed and an unzoomed state (BF1 and BF2) and say whether the projection's near scales with the zoom; build that rule.
2. `grey_probe.py`-style render of myg1_zoom 3_zoomright: std > 20 and the PS2's tracks in place (pair image in the report); the real-loop replay's tick 228 not black.
3. A census: every zoomable turret (213 on 32 maps, `lead_tmp/audit-vturz/audit_census.rs` lists them) x a look grid (yaw step 15, pitch at the limits and 0), zoomed frames whose picture depends on the near plane - count before / after (after = 0 at the PS2 value).
4. Gates: goldens unchanged or declared (third-person eyes are rarely within 0.5 m of a surface; the soldier scope eye is 0.3 / 0.6 m from the fire point - check it).
No implementation or acceptance run. Restore zoomed/unzoomed BF1/BF2 states, audit-vturz grey/replay/census inputs, turret map bakes and GPU device.
Exact checked paths (some diagnostic candidates), file counts and controls: `lead_tmp/planner_20261024_0913/queue_preflight.json`;
coordinator repeat: `lead_tmp/planner_20261024_0913/coordinator_preflight.json`.
Original acceptance remains required; skipped/empty-data tests and historical receipts do not pass it.
After restoration recheck inputs, reproduce current-main baselines and use an isolated build with full applicable gates.
