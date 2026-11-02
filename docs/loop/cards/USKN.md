---
id: USKN
title: Per-class skins and size: OverrideTexture / OverrideTexture2 (sharpshooter red, assault/assassin droid blue/red, urban/desert/snow rebels, snow Wookiee, the locals) and SkeletonRootScale / CollisionRootScale (Wookiee 1.15, Gungan 1.1, Jawa 0.6, Ewok 0.5) - the port reads neither
milestone: M1
value: 5
phase: build
status: done
deps:
files: engine/crates/bake/src/soldier.rs, engine/crates/mungefs/src/anim.rs, engine/crates/game/src/animator.rs, engine/crates/game/src/soldier_vis.rs, engine/crates/game/src/roster.rs, engine/crates/game/src/assets.rs
declares: rebake=side; goldens=units/bf1_all_inf_wookie.png,units/bf1_cis_inf_bdroid.png,units/bf1_cis_inf_sbdroid.png,units/bf2_all_inf_marksperson.png,units/bf2_all_inf_markspersonsnow.png,units/bf2_all_inf_wookie.png,units/bf2_cis_inf_bdroid.png,units/bf2_cis_inf_magnaguard.png,units/bf2_cis_inf_sbdroid.png
needs:
effort:
branch: uskn
resume: landed c5110943 2026-10-20
created: 2026-09-29
alpha: yes
---

## Why (RE 2026-10-16, analysis/re-soldier-skeleton-skins.md [O])
Two visible gaps on every map, one of them on every Clone Wars battle:
- **Own-skeleton classes are drawn at human bone lengths.** The PS2 poses a class that names a `SkeletonName` with that
  rig's bone LENGTHS (BF1 22 body bones, BF2 every bone but bone 0 / bone_root / hp_weapons take the rig's rest
  translation) and scales only bone_root's clip translation by `SkeletonRootScale`. The port copies the human clips'
  translations onto every bone (`bake::soldier::clip_to_tracks`). Idle head height port -> PS2: Wookiee 1.605 -> 1.852 m,
  MagnaGuard 1.643 -> 1.865 m, battle droid 1.605 -> 1.726 m (head sunk into the pack, shoulders 5-7 cm out), also the
  super battle droid and BF2's snipers (`lead_tmp/uskn/pics/close_a.png`, `close_b.png`). The clone trooper is unchanged.
- **Per-class skins** (OverrideTexture / 2) are not drawn: sharpshooter red, the droids' blue / red / orange / camo, the
  urban / desert / snow rebels, the snow Wookiee (unit-audit-bf1 / bf2 §3.3).

## Evidence
- The rule: re-soldier-skeleton-skins.md §1-§3 (pose builders, BF1 `FUN_0022f428` / BF2 `FUN_00281e68`, the setters); PS2
  RAM of 6 save states agrees for every fielded rig (§5, `lead_tmp/uskn/ram_bf1.txt`, `ram_bf2.txt`); disc rig tables
  `rigs_bf1.txt` / `rigs_bf2.txt` (§4); one rig and one scale per soldier model in both games (`census_model_rig.txt`).
- The override draw: BF1 `FUN_002013a0` (the live soldier; NOT `FUN_001be1e8`), BF2 `FUN_00239bb0`; OverrideTexture2 only
  with OverrideTexture in both games (§9). 13 of 18 override textures checked are not in the bake.
- Tools for the builder: `lead_tmp/uskn/tools/ps2rule_glb.py` (a GLB re-posed by the rule), `fk.py` (bone positions at a
  clip frame), `crc.py`, `bonerecs.py`.

## Notes
- UAUD2 (2026-09-29, analysis/unit-audit-bf2.md §3.3): BF2's draw is the twin `FUN_00239bb0` (B2 155278-155296, class at entity+0x4a4):
  OverrideTexture (setter 171503, class+0x844) -> group fnv("override_texture"); OverrideTexture2 (+0x848) only INSIDE
  that branch. 9 BF2 classes change their look: the CIS assault / engineer / assassin droids red / orange / camo on all
  16 CW scripts (PS2: `lead_tmp/unit-audit/pcsx2_bf2/cis_droid_skins.png`), `all_inf_rifleman` desert tan on 7 GCW
  scripts (PS2: `pcsx2_bf2/cor1g_sheet.png` vs `end_sheet.png`), `all_inf_rifleman_urban`, hot1's snow rebel faces,
  snow vanguard and snow Wookiee, geo1's `rep_inf_ep2_sniper` (red sharpshooter). SkeletonRootScale: the MagnaGuard
  (`cis_inf_officer`, every CW battle) 1.24, the Wookiee 1.15 (+ CollisionRootScale), the rebel snipers 0.95. Table in
  analysis/unit-audit-bf2.md §3.3.
