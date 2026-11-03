---
id: VTURW
title: A vehicle's gunner seat turns by the PS2's MountedTurret law (card VTURL built it for building turrets only): the slewed stick rate at MaxTurnSpeed / TurnRate, PitchLimits in the turret's nose-down sign, the aimer chain (a yaw aimer feeding a pitch aimer, e.g. BF1 combat speeder turret_body -> turret_aimer), one aimer per weapon (AT-ST, AT-TE side guns) and BF2's 0..360 yaw mode (AT-TE TURRET2 YawLimits 120 230)
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
Card VTURL (2026-10-15) put building turrets on the PS2's law (`core::vehicle::gun::Turn`, `Aimer::chase`, `vehicle_pads::turret_law`). A vehicle's TURRET section is the same MountedTurret class on the PS2 (re-vehicles 5) but the port still moves its look at the aimer's rates inside YawLimits / PitchLimits read +up, from the seat key `YawRate` that neither game's setter reads (hashloc: 0 hits; one tool, so [H]). Vehicle sections author what the buildings do not: two AimerNodeName in one section (BF1 all_hover_combatspeeder: turret_body yaw-only, then turret_aimer pitch-only), an aimer per weapon (imp_walk_atst: aimer_Reargun / aimer_Leargun with their own AimerYawLimits), PitchRate / TurnRate / filters (rep_walk_atte TURRET1 0.75 / 3.75), and BF2's K+0x844 yaw mode for limits in (0, 2pi) (B2:191619-191640; rep_walk_atte TURRET2 120 230).
## Evidence
`analysis/re-turret-draw.md` 2.2-2.3; the setter B2 FUN_00270770 (B2:191365-191760), B1 FUN_00225f08; the aimer chain walk in B1 FUN_0026e260 (B1:192436, the +0x364 next pointer); `lead_tmp/vturd/turcensus2.py bf1|bf2 --veh --dump <class>` for each vehicle section's keys.
## Acceptance
RE first: the aimer chain's base frames (does the second aimer ride the first?) and the 0x844 mode's clamp, verified; then per vehicle gunner seat the look's rate and limits against a PS2 capture of one vehicle gunner (an AT-TE or combat speeder gunner, `lead_tmp/vturd/ps2/turret_b2x.py` adapted), within 10 %.
