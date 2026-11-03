---
id: VTURA
title: Building turrets turn, fire and let go like the PS2's (VTURB audit): continuous yaw where the PS2 wraps (unauthored and ±180° YawLimits are the MountedTurret's, not the aimer's), the aimer turns the short way, a barrel with no FirePointName fires from its BarrelNodeName, the exit ray ignores only the building
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
AUDIT of VTURB (2026-09-29, PARTLY): VTURB's building turrets spawn right, but they turn, fire and let the gunner out
unlike the PS2. A player in BF1 HOT1's Hoth turret can aim only ±30° off its heading, where the PS2 turns it through
360°. Every other BF1 building turret hits a hard stop directly behind, where the PS2 keeps turning. At that stop the
port's aimer swings the long way, so bolts fly up to ~165° off the camera.

## Evidence
Report `engine/artifacts/loop/reports/20260929_0545_audit_VTURB.md`; scratch `engine/artifacts/lead_tmp/audit-vturb/`.
One Opus verifier: 12/12 SUPPORTED.
1. **Yaw limits.** A TURRET section's `YawLimits` / `PitchLimits` are the MountedTurret's own fields (B1 FUN_00225f08
   B1:138955-138976, 139032 → +0x1124/+0x1128, +0x112c/+0x1130, radians). They are not the aimer's: the aimer's are
   `AimerYawLimits` / `AimerPitchLimits` (FUN_0026e260). The ctor defaults both pairs to ±10.0 (FUN_00225cd8
   B1:138859-138868, instrs 0x225d9c-0x225de8). The tick FUN_00223850 (B1:137499-137517, instrs 0x2241a0-0x224208)
   clamps yaw only when flag +0x114c is set (0 < min ≤ π ≤ max < 2π) or when min > -3.1416926 and max < 3.1414928.
   Otherwise it only wraps into ±π. So an unauthored YawLimits and "-180 180" both turn continuously.
   - The port reads YawLimits into the one seat Aimer, else the aimer ctor's ±30°. It clamps the look there
     (`core::sim::guns::seat_gun`).
   - Measured (`audit_turret_sweep`, `sweep_hot1.txt`): Hoth turret #0 stops at ±30.0°; dish turrets stop at ±180°.
2. **Aimer wrap.** `Aimer::turn` steps `(want - cur)` without wrapping. At the 180° stop the target's yaw flips sign
   (dish #15: look -180°, aim -129.9° → -15.3° one second later).
3. **Fire point.** With no `FirePointName`, `BarrelNodeName` fills the barrel's fire-point slot too (FUN_0026e260
   B1:192569 vs 192606, verified). So [D] the PS2 fires from the barrel node: Hoth turret `lgun`, dish `barrel`, YAV1
   tower `barrel`.
   - The port instead uses `hp_fire` or else the turret node. That is off by 1.60 / 1.80 / 0.70 m in the bind pose.
   - Its citation, FUN_002864f0 B1:210168, is the weapon's own model's node (weapon+0x4c), not the turret's.
   - [H]: the fire-time reader of barrel+4 was not traced.
4. **Exit.** FUN_0020b918's path ray passes the seat's entity as a one-entry ignore list: FUN_00161cb8(…, &slot, 1,
   0x4c), instrs 0x20bc58, 0x20bc9c-0x20bcbc. FUN_00162b60 skips objects owned by it. For a building turret that
   entity is the building (FUN_002268d0 → turret+0x398).
   - So the PS2 ignores the building only, where the port skips the whole path test (`seats.rs` `paths`).
   - A turret exit can place the soldier past another wall.

## Acceptance
- A seat's turn follows the MountedTurret rule. A section whose (min, max) fails the tick's test wraps (continuous);
  otherwise it clamps, and a wrap-flag range clamps across ±π.
- The aimer's own `AimerYawLimits` / `AimerPitchLimits` (ctor ±30° / ±45°) bound only the gun's convergence.
- The aim turns the short way.
- A barrel with no FirePointName fires from its BarrelNodeName.
- The exit's path test ignores the turret's own building bodies, not all bodies.
- Tests:
  - the HOT1 Hoth turret and a dish turret sweep 360° (`turret_ride` or a unit test);
  - the aim never swings more than rate·dt past the look;
  - the Hoth turret's first bolt leaves at `lgun`;
  - an exit beside a wall is refused through it.
- Vehicle gunner seats share the code: 12 BF1 / 70 BF2 sections author -180 180. Declare what moves (expected none —
  seat guns are human-only until VTAI); `turret_ride` replay hash EQUAL.

## Planner note (2026-09-30, M-ALPHA)
`alpha: yes` — turrets that turn and fire right. Value 4 (kept). Showcase maps: BF1 NAB1, BF1 TAT1 (the port's `tat1`), BF1 YAV2, BF2 UTA1, BF2 MYG1, BF2 HOT1 (PLAN §Milestones M-ALPHA).

## Current-source reconciliation (2026-10-24 10:33 UTC)
Landed VTURL provides turret_law (vehicle_pads.rs:883-907), separate Aimer keys and Turn, and building-gun Aimer::chase (sim/guns.rs:350-373). The older turn-gap premises are historical for building turrets; retain their original tests as regressions. Source still uses hp_fire/turret fallback rather than BarrelNodeName (vehicle_pads.rs:849-857), and skips the turret exit path (sim/seats.rs:441). Re-measure these remaining cases before changing them. TCOL is a functional prerequisite for the exit test to ignore owned building bodies while still refusing another wall; do not rewrite VTURL or broaden this to all vehicle seats without a trace.
