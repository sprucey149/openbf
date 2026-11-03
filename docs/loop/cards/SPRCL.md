---
id: SPRCL
title: No soldier freezes at sprint: a census of every class on both games for the clip the animator picks while sprinting (some bodies stay in the idle pose at sprint speed), the PS2's fallback for a set without a sprint clip, every class fixed and the census kept as a test
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
The maintainer (INBOX 2026-10-21 17:50 item 2): some characters' bodies freeze in the idle pose while moving at sprint speed.
ANIML says "no action clip in ... sprint". Related: BLKS (the black box's sprint inputs), CSPD (BF2 per-stance turn).

## Do
1. Census (like `layer_census`): per class / skeleton, the clip picked at sprint speed, on both games.
2. RE the PS2's pick when a set has no sprint clip (the fallback table, ANIMS).
3. Fix every class.

## Acceptance
- The census line: classes with no moving clip at sprint, N -> 0, kept as a test over every baked class so a new class
  cannot freeze; a picture strip of one fixed class beside the PS2.

## RE findings (2026-10-22, `analysis/re-sprint-clip.md`, docs 672d943)
- Measured on main f44cdcf (port census, `lead_tmp/sprcl/census_main.tsv`): BF1 140 / 146 class-db (class, weapon) rows
  and 100 / 104 fielded (class, body, set) rows sprint in `*_stand_idle_emote` (only the Wookiees move); BF2 0 / 324,
  0 / 104. Cause: BF1 has no sprint on the PS2 (ours), and the port keeps BF1's chooser rule "over 8 m/s, idle"
  (B1 FUN_0022e990, `64.0 < v^2`, read before the root scale).
- BF2's PS2 has a sprint pair in every set (0 of 2,777 RAM set rows empty) and plays it on both players at
  |v| / the lower sprint clip's root speed (B2 FUN_0027caf0 case 0); the port plays it at 1.0 (live: 118 PS2 updates,
  port 0.009 of phase per update off, the rule 0.00005).
- Bars for the build (knock-in measured in `lead_tmp/sprcl`): census BF1 140 -> 0 (and 100 -> 0), BF2 stays 0; the PS2's
  118 sprint rows through the port: mean |phase step - PS2| 0.00902 -> 0.00004 (bar: under 0.0002); each test fails
  under its mutation; picture strip: port BF2 clone trooper sprint beside `ps2/out/bf2_sprint.mkv`, port BF1 trooper
  sprint beside the PS2 BF1 run `ps2/out/bf1_run.mkv`.

## AUDIT findings (2026-10-22, lane 1, `analysis/re-sprint-clip.md` §7, docs 8a3680b) - add to the build and its acceptance
- The law holds, measured on BOTH players (412 live pairs each, error 0.000000): rate = |m_xz| / v_clip(LOWER), the
  upper runs at that rate whatever its own clip (pistol sprint: upper `human_pistol_stand_runforward` 19 f at the lower
  `human_rifle_sprint_full`'s rate), and |m_xz| is the planar MAGNITUDE (not the forward component).
- (1) Root twist: in `Body::Special(Sprint)` the PS2 eases the root twist toward atan2(x, z) of the local planar velocity
  by min(1, 7.5 dt) per update (B2 FUN_0027d9a8 201534-201548 -> FUN_00280048 yaw on dummyroot); the port eases it to 0
  (`animator.rs` ~1255-1260). Build it; test with a diagonal fixture cut from `lead_tmp/audit-sprcl/ps2/run1/bf2_diag.pkl`
  and `run2/bf2_strafel.pkl` (decoder `audit-sprcl/tools/sdec.py`; travel -14.04 / +14.04 deg, the twist's per-update
  steps); mutation: target 0 must fail.
- (2) Droid fallback: v_clip = the lower clip's forward root speed, else 11.5 m/s when that is <= 0 (B2 FUN_00282900
  204683-204695, the engine's id-0 default) - NOT 1.0. The port bakes bdroid_rifle / bdroid_pistol / sbdroid_rifle
  `_sprint_full` with root_motion 0, so today's knock-in plays them at 1.0 where the PS2 plays |v| / 11.5.
- (3) The planned replay test (one class, rifle, straight, scale 1) cannot catch: the upper clip's own speed, the forward
  component, a scale division, the droid fallback. Add a pistol-sprint fixture (`audit-sprcl/ps2/run1/bf2_r2.pkl`, upper
  phase) and a diagonal one, and a droid rig case for the fallback. The PS2 divides by whole cm/s (9.16, not the bake's
  9.1627): optional, 0.00002 of phase per update.
