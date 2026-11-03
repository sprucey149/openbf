---
id: FPV
title: The PS2's first-person view on foot: the class's FPM arms (FPM/<side>/<class>.LVL, e.g. imp_1st_inf_stormtrooper_arms / _hands) drawn at the eye, posed by the first-person clip table (BF1 _1st_<set>_<action> in human_animation_fp, BF2 humanfp_*: idle, run, shoot, reload, jump, throw...), and the toggle into it
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
The maintainer (INBOX 2026-10-21 17:50 item 1) asked for the grenade throw 'in first and third person'. Card GTHRW's RE
(`analysis/re-grenade-throw-anim.md` 4) found the PS2's first-person throw clips - BF1 `_1st_grenade_shoot2`, BF2
`humanfp_grenade_shoot2` (11 frames) - but the port has no first-person view on foot at all (only the scope view,
which draws no body), so the first-person half of GTHRW waits on this card. Both PS2 games have the view: an FPM arm
model per class (FPM LVLs, 66 BF1 / 86 BF2 files), an 11-column clip table per weapon row (BF1 `FUN_001a6880` /
`FUN_001a38e0`, BF2 0x618440 / `FUN_001beeb8` / `FUN_001bfcb0`), a view record flag (`re-aim-assist.md` 10.2:
ScriptCB_SetFirstPersonView, the soldier's toggle `FUN_00204578`, a class that forces a view). The default pad has no
toggle (pcsx2 README): how a player enters it on the PS2 is part of the RE.

## Acceptance
- RE: how the view is entered (pad / menu / script / class), the camera (eye point, FOV, the FPM model's placement),
  the FPM model format and which class loads which model, the first-person animator's rules (rows, columns, blends,
  the bob) beyond GTHRW's section 4; a live PCSX2 picture of a first-person throw (write the view record's flag).
- Built: a toggle into first person; the arms drawn and posed; a test that a throw plays the 11-frame throw clip on
  the arms from the press; a picture strip per game beside the PS2's.
