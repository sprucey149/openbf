---
id: TAIM
title: Bots in BF2's recoilless turrets (MYG1, KAS2) and BF1 Geonosis' geoturret never fire: the barrel never comes inside the gunner's 2-degree cone - the aimer's pitch sits ~0.35 rad above the line to the target (found by card TURR)
milestone: M3
value: 4
phase: build
status: done
deps: TURR
files: engine/crates/core/src/ai/gunner.rs, engine/crates/core/src/sim/guns.rs
declares: sim; sweep; arena
needs:
effort:
branch:
resume: MERGED into TURR 2026-10-16 (planner 1016b): built as TURR's step A on branch turr, because TURR holds until its gunners fire (Q56 default). Re-open (status=todo, deps=TURR) if the maintainer takes TURR's kill cost and it lands without the fix.
created: 2026-10-16
---
## Why
Card TURR (branch turr, 33016bf) has bots man building turrets. On three maps they sit and never fire (seed 1, `ROUND_VERBOSE=1 round_to_outcome 36000 1 <map>`, the `turrets:` line): BF2 MYG1 21 spells, 0 with a shot; BF2 KAS2 8 spells, 0; BF1 GEO1 50 spells, 4 with a shot (0.2 bolts a seated minute). TURR's exit 2 (45 s seated, no shot in 20 s) puts them out, so the cost is bounded, but a gunner that never fires is the purposeless camping the maintainer named.

## Evidence
`engine/artifacts/lead_tmp/turr/myg_dbg.txt` (an instrumented scratch of the branch, `gunner::aim`'s fire gate per tick for turret gunners with a SEEN target, BF2 MYG1 seed 1, class `tur_weap_recoilless_rifle` on `tur_bldg_recoilless_lg`): 1,910 rows, `ready` true on 1,587, the pause open on 1,909, `on` (barrel within `AIM_TOLERANCE` = 0.035 rad of muzzle->target) on NONE - barrel/target dot median 0.871 (29 deg), best 0.992 (7 deg). The look's pitch tracks the target's (-0.10 against -0.11 rad) while the aim (barrel) pitch sits at +0.23..+0.25: the aimer (`Sim::seat_gun`, `Aimer::chase` toward the 512 m camera ray's point) does not bring the barrel onto the line. A human in the same turret (`turret_ride bf2_myg1 0`) shows aim 4.8 deg over a level look. A variant that tests the cone on the camera look instead (`TURR_VIEW`, `lead_tmp/turr/view/`) fires but does not hit: MYG1 0 hits, KAM1 bolts a seated minute 15.5 -> 60 with hits 16 -> 14 - so the aim itself is off, not only the gate.

## What to find / build
1. Why the recoilless aimer's barrel pitch stays ~0.35 rad over the camera line (the class's AimerPitchLimits / EyePointOffset / fire point, or the camera ray hitting the turret's own building body before the target - `aim_point` skips the vehicle's hull, not a map body). Read the class props and dump one tick's camera ray hit.
2. Fix it where it lives (the seat gun if a human's shots miss too - check a human's hit there; else the bot's look), BF1 Geonosis' geoturret likewise (rest aim pitch 45 deg).
