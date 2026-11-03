---
id: DEKD
title: Draw the droideka: bake cis_walk_droideka with its 12 clips and bodies, pose it by state (walk steps, fold, the spinning ball, unfold), the shield bubble, fold / unfold / step / engine sounds
status: archived
issue: 85
---

Historical technical record. Recorded state before archival was `todo`. Current scope and ownership are on [issue #85](https://github.com/sprucey149/openbf/issues/85).

## Why
`analysis/re-droideka-unit.md` section 4 [O]: the model, its bank and its states are known; the port bakes neither. A unit the sim holds (DEKU) must be seen before it is fielded (DEKF).

## Evidence
- Model `cis_walk_droideka` (SIDE/CIS.LVL, both discs), low-res models `cis_droideka_idle_low1` / `cis_droideka_roll_low1`, bank `cis_walk_droideka` (clips idle 46 frames, turnl / turnr 9, idle_to_leftfoot 17, walk_leftfoot_rightfoot / walk_rightfoot_leftfoot 17, leftfoot_to_idle / rightfoot_to_idle 24, fold 31, roll 11, unfold 101, death01 81; names stored as the complemented CRC-32/MPEG-2), bones DummyRoot / center_box / bone_spine1..3, fire points hp_gun_1 / hp_gun_2.
- Clip per state and its rate: section 4; backwards = the walk clips in reverse; the ball's spin and lean come from the sim (re-droideka 2).
- Shield: `ShieldEffect droidekashield` (CIS.LVL's one `fx__` Shield) at ShieldOffset, r 1.25; DEKA's DSHN note: BF1 draws it with the `specular` table through its own records (FUN_00389338; lead_tmp/audit-dshn).
- Sounds: EngineSound `cis_walk_droideka_engine_parameterized`, `cis_droideka_fold` / `cis_droideka_unfold`, the three `cis_inf_droideka_step` keys, fire `cis_weap_inf_droideka_fire`, death `imp_weap_ord_exp_lg`; chunks droideka_chunk1 / 3 / 4 / 5 with ExplosionName.
- PS2 pictures: `lead_tmp/unit-audit/pcsx2/deka_sheet.png`; `lead_tmp/deka/live/*_t0.png`.

## Acceptance
(1) A bake test: both discs' droideka GLB holds the 12 named clips with the frame counts above and the 16 bodies of section 7.2. (2) A new golden: a droideka in idle, mid-fold, ball and shield-up poses on one sheet (declared when it lands), viewed beside `deka_sheet.png`. (3) A test: the pose clip per state equals section 4's table and the walk clips play in reverse on a backward walk. (4) SOUND_LOG census: fold, unfold and a step sound start once per event in a scripted fold / walk / unfold.

## Notes
- First person (`droidekafp`, 4 clips) is not in scope.
- 2026-10-06 planner: `engine/CRATES.md` taken out of `files:`. The picker refuses a card whose files overlap a card in progress, and 22 open cards listed the map, so any one of them held the other 21 (a chain card in progress held WADE and WTHR). The map's row for a new or moved source file is still owed in the same branch (`scripts/size_report.sh --check-map`); a rebase conflict in it is one line. Undo: add the path back.

## Planner note
Planner 2026-10-21 20:50 (the maintainer via INBOX 17:50 item 3: DEKF + DEKD before DEKS if the shield blocks the draw): dep DEKS dropped, `first: 7`. The shield bubble's draw moves to DEKS; this card draws the body, its clips and sounds.
