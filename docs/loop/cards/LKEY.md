---
id: LKEY
title: Weapon look keys read the LAST of a repeated line, as the PS2's setters do (NODMG part 1 fixed the sim keys only): 88 look-key repeats still read first-wins - every BF1 / BF2 pistol bolt's GlowLength 0.75 where the disc's later line says 3, the rifle's flash colour 255 80 80 vs 255 100 100 - check the look setters' order in both ELFs, then move the laser / flash / impact / sound readers to prop_last
status: archived
---

Historical technical record. Recorded state before archival was `todo`.

## Why
NODMG part 1 (branch nodmg 7a19427, report `reports/20261021_2148_card_NODMG.md`): `ClassDb::prop_last` / `roster::ScalarSrc`
give the sim readers the PS2's last-wins order (B1 FUN_002738a8, B2 FUN_00310620: each PROP line stored into the field in
file order). The look readers (laser, flash, impact, HUD icon, sounds) were left first-wins on purpose so NODMG moves no
golden. `lead_tmp/nodmg/p1/lastwins.py` lists 102 key moves over all keys, 88 of them look keys. Player-visible: every
pistol bolt's glow is a quarter of its length on the disc's last line.

## Do
1. Check that the look keys go through the same setters (the laser / flash class parsers' PROP loops, both ELFs; RE
   discipline: `re_check.py fn` on each cited line). A key the PS2 reads by section (lists) stays as it is.
2. Move the look readers to `ScalarSrc`; regenerate the look goldens that move and view each.

## Acceptance
- The census test of NODMG extended to the look keys: exactly the listed classes move, each named.
- A PCSX2 frame of a BF2 pistol bolt in flight (or BF1's) beside the port's, glow length within 10 %.
## Notes
